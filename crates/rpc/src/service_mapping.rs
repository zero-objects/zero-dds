// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Service IDL → typed service data model — Spec §7.4.
//!
//! The DDS-RPC spec maps an IDL service definition
//!
//! ```text
//! @service interface Calculator {
//!     long add(in long a, in long b);
//!     void log(in string msg);
//! };
//! ```
//!
//! onto two wire structures per method:
//!
//! * `<Service>_<Method>_In`  — request payload (all `in`/`inout`).
//! * `<Service>_<Method>_Out` — reply payload (return + `out`/`inout`).
//!
//! This foundation stage (C6.1.A) represents only the **data model**
//! ([`ServiceDef`], [`MethodDef`], [`ParamDef`]). The actual
//! codegen stage (C6.1.B) consumes the model and emits IDL
//! structures + Rust bindings.
//!
//! The model is constructed via [`lower_service`] from one
//! [`InterfaceNode`] of the common AST. The RPC annotations are read off
//! that node's facets by [`lower_rpc_annotations`] rather than being
//! passed in: they and the interface are the same declaration, and the
//! two-argument form let a caller pair an interface with another
//! interface's annotations. Validation at this stage:
//!
//! * The service definition conforms to §7.3.1.1's grammar — no
//!   `import`, `typeid`/`typeprefix`, embedded declarations, `oneway` or
//!   `context`.
//! * The service name is non-empty + alphanumeric + `_`.
//! * Method names are unique.
//! * Parameter names per method are unique.

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use zerodds_ast::{
    Ast, InterfaceKind, InterfaceNode, OperationNode, ParamDirection as AstParamDirection, TypeNode,
};
use zerodds_ast_core::{Id, Ref};

use crate::annotations::{LoweredRpc, lower_rpc_annotations};
use crate::error::{RpcError, RpcResult};
use crate::topic_naming::{
    BindingStyle, DEFAULT_SERVICE_NAME, ServiceTopicNames, TopicNameOverrides,
    qualified_interface_name, validate_service_name, validate_user_def_alpha_num,
};

/// Direction attribute of an RPC parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamDirection {
    /// `in` — Request-only.
    In,
    /// `out` — Reply-only.
    Out,
    /// `inout` — both.
    InOut,
}

impl From<AstParamDirection> for ParamDirection {
    fn from(value: AstParamDirection) -> Self {
        match value {
            AstParamDirection::In => Self::In,
            AstParamDirection::Out => Self::Out,
            AstParamDirection::InOut => Self::InOut,
        }
    }
}

impl ParamDirection {
    /// `true` if the parameter lands in the request topic (`in` or `inout`).
    #[must_use]
    pub const fn is_in(self) -> bool {
        matches!(self, Self::In | Self::InOut)
    }

    /// `true` if the parameter lands in the reply topic (`out` or `inout`).
    #[must_use]
    pub const fn is_out(self) -> bool {
        matches!(self, Self::Out | Self::InOut)
    }
}

/// Type reference in the service model.
///
/// A handle into the model the service was lowered from, not a copy of a
/// syntax node: resolving it to a name, a layout or a codec is the
/// model's job, and every consumer that needs one already holds the
/// [`Ast`].
pub type TypeRef = Ref<TypeNode>;

/// An RPC parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamDef {
    /// Parameter name.
    pub name: String,
    /// Direction (`in`/`out`/`inout`).
    pub direction: ParamDirection,
    /// Type.
    pub type_ref: TypeRef,
}

/// One exception an operation may raise.
///
/// Two names, because §7.5.1.1.5 uses both: rule 5b builds the union
/// member out of the *simple* name — "the lower-case version of the
/// exception name with the suffix `_ex` added" — and rule 4b hashes the
/// *scoped* one, "the HASH of the fully qualified name of the exception
/// type where module separator to be used is `::` (2 colons)".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RaisedException {
    /// The exception's simple IDL name.
    pub name: String,
    /// Its scoped name, `::`-separated.
    pub scoped_name: String,
}

/// An RPC method.
#[derive(Debug, Clone, PartialEq)]
pub struct MethodDef {
    /// Method name.
    pub name: String,
    /// Parameters in declaration order.
    pub params: Vec<ParamDef>,
    /// Return type. `None` for `void`.
    pub return_type: Option<TypeRef>,
    /// `true` if `oneway` (no reply, only `in` params, void return).
    pub oneway: bool,
    /// The `raises` clause, in declaration order. §7.5.1.1.5 turns each
    /// entry into a case of the operation's `Result` union.
    pub raises: Vec<RaisedException>,
    /// `true` if the operation carries `@Mutable` (§7.5.1.2.1.4): the
    /// Enhanced Service Mapping declares its `In`, `Out` and `Result`
    /// structures `@Extensibility(MUTABLE_EXTENSIBILITY)`. The Basic
    /// mapping ignores it.
    pub mutable: bool,
}

impl MethodDef {
    /// All `in`/`inout` parameters (request fields).
    pub fn in_params(&self) -> impl Iterator<Item = &ParamDef> {
        self.params.iter().filter(|p| p.direction.is_in())
    }

    /// All `out`/`inout` parameters (reply fields).
    pub fn out_params(&self) -> impl Iterator<Item = &ParamDef> {
        self.params.iter().filter(|p| p.direction.is_out())
    }
}

/// An interface attribute, lowered. Spec §7.5.1.1.3.
#[derive(Debug, Clone, PartialEq)]
pub struct AttributeDef {
    /// Attribute name.
    pub name: String,
    /// `true` for `readonly` — no setter is implied.
    pub readonly: bool,
    /// The attribute's type.
    pub type_ref: TypeRef,
    /// Exceptions the implied getter may raise.
    pub get_raises: Vec<RaisedException>,
    /// Exceptions the implied setter may raise.
    pub set_raises: Vec<RaisedException>,
}

/// An RPC service definition.
///
/// Three names, not one. §7.5.1.1.6 builds the *type* names from
/// `${interfaceName}` — the interface's own simple name, "in the same
/// module as the interface" — while §7.4.1 builds the *topic* names from
/// the fully-qualified interface name and a separate `<service_name>`.
/// A single field for both produced `Calculator_Request` where the
/// clause asks for the type `Calculator_Request` on the topic
/// `Calculator_Service_Request`, and a conforming peer subscribed to
/// neither.
#[derive(Debug, Clone, PartialEq)]
pub struct ServiceDef {
    /// The interface's simple name — `${interfaceName}` of §7.5.1.1.4-7,
    /// which every generated type name is built from.
    pub name: String,
    /// The `<interface_name>` of §7.4.1: the enclosing module names
    /// joined by underscores, then [`Self::name`].
    pub qualified_name: String,
    /// The `<service_name>` of §7.4.1 — `@service(name = "…")` where the
    /// IDL supplies one, `"Service"` where it does not.
    pub service_name: String,
    /// `@DDSRequestTopic(name = "…")`, §7.4.2.2: a whole topic name.
    pub request_topic: Option<String>,
    /// `@DDSReplyTopic(name = "…")`, §7.4.2.2.
    pub reply_topic: Option<String>,
    /// The qualified names of the base interfaces, in declaration order.
    /// §7.4.3 turns each into a topic alias.
    pub bases: Vec<String>,
    /// Methods.
    pub methods: Vec<MethodDef>,
    /// Attributes, in declaration order. §7.5.1.1.3 implies two
    /// operations from each; see [`Self::all_methods`].
    pub attributes: Vec<AttributeDef>,
    /// The operations of every base interface, the base's own and the
    /// ones it inherits, each group under the interface that declares
    /// it, a base reached twice listed once. §7.5.1.2.6: "A derived
    /// interface is interpreted as if all the inherited operations are
    /// declared in-place in the derived interface." The Basic mapping
    /// does not read this (§7.5.1.1.8).
    pub inherited: Vec<InheritedOperations>,
}

/// The operations one base interface declares, for the flattening of
/// §7.5.1.2.6.
#[derive(Debug, Clone, PartialEq)]
pub struct InheritedOperations {
    /// The base's simple name: the `${interfaceName}` its own `In`,
    /// `Out` and `Result` types are named after (the spec's example
    /// keeps `Adder_add_In` in `Calculator_Request`).
    pub interface: String,
    /// Its declared operations followed by the ones its attributes
    /// imply, as [`ServiceDef::all_methods`] orders them.
    pub methods: Vec<MethodDef>,
}

impl ServiceDef {
    /// A service over an interface at file scope, with no
    /// `@service(name = "…")`, no topic annotations and no bases.
    ///
    /// What a definition that did not come through the IDL frontend has:
    /// `crates/rpc/src/java_service_def.rs` reads one out of Java source
    /// (§7.3.2) and has no module chain to walk. `<service_name>` takes
    /// its `"Service"` default, so the topic names are §7.4.2.1's.
    #[must_use]
    pub fn new(interface_name: impl Into<String>, methods: Vec<MethodDef>) -> Self {
        let name: String = interface_name.into();
        Self {
            qualified_name: name.clone(),
            name,
            service_name: DEFAULT_SERVICE_NAME.to_string(),
            request_topic: None,
            reply_topic: None,
            bases: Vec::new(),
            methods,
            attributes: Vec::new(),
            inherited: Vec::new(),
        }
    }

    /// Every operation of the service: the declared methods, then the two
    /// §7.5.1.1.3 implies for each attribute.
    ///
    /// Rule 1: "Each attribute in an interface maps to a pair of IDL
    /// operations `get_attribute_<attribute-name>` and
    /// `set_attribute_<attribute-name>` in the same interface." A
    /// `readonly` attribute implies the getter alone. The order is
    /// declaration order of the methods followed by declaration order of
    /// the attributes.
    #[must_use]
    pub fn all_methods(&self) -> Vec<MethodDef> {
        let mut out = self.methods.clone();
        for attribute in &self.attributes {
            out.push(MethodDef {
                name: alloc::format!("get_attribute_{}", attribute.name),
                params: Vec::new(),
                return_type: Some(attribute.type_ref.clone()),
                oneway: false,
                raises: attribute.get_raises.clone(),
                mutable: false,
            });
            if attribute.readonly {
                continue;
            }
            out.push(MethodDef {
                name: alloc::format!("set_attribute_{}", attribute.name),
                params: alloc::vec![ParamDef {
                    name: attribute.name.clone(),
                    direction: ParamDirection::In,
                    type_ref: attribute.type_ref.clone(),
                }],
                return_type: None,
                oneway: false,
                raises: attribute.set_raises.clone(),
                mutable: false,
            });
        }
        out
    }

    /// The topic names under the function-call binding with nothing
    /// specified at run time — §7.4.2.1's default mechanism.
    ///
    /// # Errors
    /// `RpcError::InvalidServiceName` for a name §7.4.1 does not admit.
    pub fn topic_names(&self) -> RpcResult<ServiceTopicNames> {
        self.topic_names_with(BindingStyle::FunctionCall, &TopicNameOverrides::none())
    }

    /// The topic names under `style`, with nothing specified at run
    /// time.
    ///
    /// # Errors
    /// As [`Self::topic_names`].
    pub fn topic_names_for(&self, style: BindingStyle) -> RpcResult<ServiceTopicNames> {
        self.topic_names_with(style, &TopicNameOverrides::none())
    }

    /// The topic names under `style`, with the run-time layer of
    /// §7.4.2.3 applied over the annotations and the default.
    ///
    /// # Errors
    /// As [`Self::topic_names`].
    pub fn topic_names_with(
        &self,
        style: BindingStyle,
        overrides: &TopicNameOverrides,
    ) -> RpcResult<ServiceTopicNames> {
        let bases: Vec<&str> = self.bases.iter().map(String::as_str).collect();
        ServiceTopicNames::synthesize(
            style,
            &self.qualified_name,
            &bases,
            &self.service_name,
            self.request_topic.as_deref(),
            self.reply_topic.as_deref(),
            overrides,
        )
    }
}

/// Refuses the IDL constructs the service-definition grammar of
/// §7.3.1.1 removed.
///
/// Productions (1), (2), (7), (9) and (87) modify IDL 3.5: `import`,
/// `<type_id_dcl>`, `<type_prefix_dcl>`, the CCM declarations,
/// declarations of types/constants/exceptions inside an interface, the
/// `oneway` modifier and `context` expressions are all gone. A service
/// definition that carries one is outside the grammar, and the two the
/// mapping used to accept silently changed the result — a constant in an
/// interface vanished, and a `oneway` operation lost its reply structure
/// and its Return-union case. Reporting the first offending construct is
/// the alternative to skipping it.
///
/// `oneway` is refused as a whole: §7.5.1.1.7 rule 5 requires a Return
/// union case for *each* operation, and neither the IDL keyword nor the
/// `@oneway` annotation is in the Basic grammar (§7.3.2 keeps `@oneway`
/// for Java source only).
///
/// # Errors
/// [`RpcError::OutsideServiceGrammar`] for the first construct found.
fn reject_outside_service_grammar(ast: &Ast, iface: &InterfaceNode) -> RpcResult<()> {
    // Production (7): `<interface_header> ::= … "interface" <identifier> …`.
    // `local`/`abstract` interfaces and the CCM forms are not in it.
    if iface.kind != InterfaceKind::Interface {
        return Err(RpcError::OutsideServiceGrammar {
            construct: "an interface declaration of a form other than `interface`",
            at: alloc::format!("{} {}", iface.kind.as_str(), iface.name),
        });
    }

    // Productions (1) and (2): the file-scoped directives, kept on the
    // root module because they leave no node of their own.
    let root = ast.root_module();
    if let Ok(module) = ast.module(root) {
        if let Some(corba) = module.facets.corba.as_ref() {
            if let Some(first) = corba.imports.first() {
                return Err(RpcError::OutsideServiceGrammar {
                    construct: "an `import` directive",
                    at: first.clone(),
                });
            }
            if let Some(first) = corba.repository_directives.first() {
                return Err(RpcError::OutsideServiceGrammar {
                    construct: "a `typeid`/`typeprefix` directive",
                    at: first.clone(),
                });
            }
        }
    }

    // Production (9): an interface body holds attributes and operations
    // only. A nested declaration is still lowered into the model — IDL
    // 4.2 admits it — so it is found by its declaration scope.
    let nested_scope = Some(iface.name.as_str());
    let is_nested = |module: Id<zerodds_ast::ModuleNode>, scope: Option<&str>| {
        module == iface.module && scope == nested_scope
    };
    for (_, node) in ast.types() {
        if is_nested(node.module, node.scope.as_deref()) {
            return Err(RpcError::OutsideServiceGrammar {
                construct: "a type declaration in an interface body",
                at: node.name.clone(),
            });
        }
    }
    for (_, node) in ast.constants() {
        if is_nested(node.module, node.scope.as_deref()) {
            return Err(RpcError::OutsideServiceGrammar {
                construct: "a constant declaration in an interface body",
                at: node.name.clone(),
            });
        }
    }
    for (_, node) in ast.exceptions() {
        if is_nested(node.module, node.scope.as_deref()) {
            return Err(RpcError::OutsideServiceGrammar {
                construct: "an exception declaration in an interface body",
                at: node.name.clone(),
            });
        }
    }
    Ok(())
}

/// Lowers one interface of the model into the typed service model.
///
/// The RPC annotations are read off the interface's own facets. If
/// `@service(name = "...")` is not set, the service name is `"Service"`
/// (§7.4.1); the interface name is a segment of the topic names of its own.
///
/// # Errors
/// * `RpcError::StaleHandle` when a handle does not address a live node.
/// * `RpcError::OutsideServiceGrammar` for a construct §7.3.1.1 removed.
/// * `RpcError::InvalidServiceName` for an empty or invalid name.
/// * `RpcError::InvalidMethodName` for an empty method name.
/// * `RpcError::DuplicateMethod` / `DuplicateParam` on name collisions.
pub fn lower_service(ast: &Ast, id: Id<InterfaceNode>) -> RpcResult<ServiceDef> {
    let iface = ast
        .interface(id)
        .map_err(|_| RpcError::StaleHandle { what: "interface" })?;
    reject_outside_service_grammar(ast, iface)?;
    let lowered = lower_rpc_annotations(ast, &iface.facets);

    // §7.4.1: `<service_name>` is what `@service(name = "…")` supplies,
    // and `"Service"` where it supplies nothing. It is *not* the
    // interface name — that is a separate segment of the topic name and
    // the whole of every type name.
    let service_name = lowered
        .service_name()
        .map_or_else(|| DEFAULT_SERVICE_NAME.to_string(), ToString::to_string);
    validate_service_name(&service_name)?;

    let qualified_name = qualified_name_of(ast, iface)?;
    validate_user_def_alpha_num(&qualified_name)?;

    // §7.4.3: each parent interface contributes a topic alias, built
    // from its own qualified name.
    let mut bases = Vec::with_capacity(iface.bases.len());
    for base in &iface.bases {
        let base_id = base.resolved().ok_or(RpcError::StaleHandle {
            what: "base interface",
        })?;
        let base_node = ast.interface(base_id).map_err(|_| RpcError::StaleHandle {
            what: "base interface",
        })?;
        bases.push(qualified_name_of(ast, base_node)?);
    }

    // §7.5.1.1.3: attributes are lowered too, so `all_methods` can imply
    // their get/set operations. Constants and nested types are not part
    // of the service model.
    let mut attributes = Vec::with_capacity(iface.attributes.len());
    for attr in &iface.attributes {
        let attr = ast
            .attribute(*attr)
            .map_err(|_| RpcError::StaleHandle { what: "attribute" })?;
        attributes.push(lower_attribute(ast, attr)?);
    }

    let mut methods = Vec::with_capacity(iface.operations.len());
    for op in &iface.operations {
        let op = ast
            .operation(*op)
            .map_err(|_| RpcError::StaleHandle { what: "operation" })?;
        methods.push(lower_method(ast, op)?);
    }

    // Duplicate-Method-Check, over declared methods and the operations
    // the attributes imply. §7.5.1.1.3 rule 1: "It is illegal to have an
    // interface with an attribute named <attribute-name> and
    // user-defined operations named get_attribute_<attribute-name> and
    // set_attribute_<attribute-name>."
    let every = {
        let service = ServiceDef {
            name: iface.name.clone(),
            qualified_name: qualified_name.clone(),
            service_name: service_name.clone(),
            request_topic: None,
            reply_topic: None,
            bases: Vec::new(),
            methods: methods.clone(),
            attributes: attributes.clone(),
            inherited: Vec::new(),
        };
        service.all_methods()
    };
    for i in 0..every.len() {
        for j in (i + 1)..every.len() {
            if every[i].name == every[j].name {
                return Err(RpcError::DuplicateMethod(every[i].name.clone()));
            }
        }
    }

    let mut inherited = Vec::new();
    collect_inherited(ast, iface, &mut Vec::new(), &mut inherited)?;

    Ok(ServiceDef {
        name: iface.name.clone(),
        qualified_name,
        service_name,
        request_topic: lowered.dds_request_topic().map(ToString::to_string),
        reply_topic: lowered.dds_reply_topic().map(ToString::to_string),
        bases,
        methods,
        attributes,
        inherited,
    })
}

/// The deepest inheritance chain the flattening of §7.5.1.2.6 follows.
const MAX_INHERITANCE_DEPTH: usize = 64;

/// Collects the operations of every base of `iface`, depth first, each
/// base once however many paths reach it. §7.5.1.2.6.
///
/// zerodds-lint: recursion-depth 64 (each base is entered once; the list of
/// bases seen is capped at `MAX_INHERITANCE_DEPTH`)
fn collect_inherited(
    ast: &Ast,
    iface: &zerodds_ast::InterfaceNode,
    seen: &mut Vec<String>,
    out: &mut Vec<InheritedOperations>,
) -> RpcResult<()> {
    for base in &iface.bases {
        let base_id = base.resolved().ok_or(RpcError::StaleHandle {
            what: "base interface",
        })?;
        let base_node = ast.interface(base_id).map_err(|_| RpcError::StaleHandle {
            what: "base interface",
        })?;
        let qualified = qualified_name_of(ast, base_node)?;
        if seen.contains(&qualified) {
            continue;
        }
        if seen.len() >= MAX_INHERITANCE_DEPTH {
            return Err(RpcError::OutsideServiceGrammar {
                construct: "an interface hierarchy of more than 64 interfaces",
                at: qualified,
            });
        }
        seen.push(qualified.clone());
        collect_inherited(ast, base_node, seen, out)?;

        let mut attributes = Vec::with_capacity(base_node.attributes.len());
        for attr in &base_node.attributes {
            let attr = ast
                .attribute(*attr)
                .map_err(|_| RpcError::StaleHandle { what: "attribute" })?;
            attributes.push(lower_attribute(ast, attr)?);
        }
        let mut methods = Vec::with_capacity(base_node.operations.len());
        for op in &base_node.operations {
            let op = ast
                .operation(*op)
                .map_err(|_| RpcError::StaleHandle { what: "operation" })?;
            methods.push(lower_method(ast, op)?);
        }
        let declared = ServiceDef {
            name: base_node.name.clone(),
            qualified_name: qualified,
            service_name: DEFAULT_SERVICE_NAME.to_string(),
            request_topic: None,
            reply_topic: None,
            bases: Vec::new(),
            methods,
            attributes,
            inherited: Vec::new(),
        };
        out.push(InheritedOperations {
            interface: base_node.name.clone(),
            methods: declared.all_methods(),
        });
    }
    Ok(())
}

/// Lowers one attribute of an interface. §7.5.1.1.3.
fn lower_attribute(ast: &Ast, attribute: &zerodds_ast::AttributeNode) -> RpcResult<AttributeDef> {
    Ok(AttributeDef {
        name: attribute.name.clone(),
        readonly: attribute.readonly,
        type_ref: attribute.type_.clone(),
        get_raises: lower_raises(ast, &attribute.get_raises)?,
        set_raises: lower_raises(ast, &attribute.set_raises)?,
    })
}

/// Resolves a `raises`/`getraises`/`setraises` list to its scoped names.
fn lower_raises(
    ast: &Ast,
    raised: &[Ref<zerodds_ast::ExceptionNode>],
) -> RpcResult<Vec<RaisedException>> {
    let mut out = Vec::with_capacity(raised.len());
    for entry in raised {
        let id = entry.resolved().ok_or(RpcError::StaleHandle {
            what: "raised exception",
        })?;
        let node = ast.exception(id).map_err(|_| RpcError::StaleHandle {
            what: "raised exception",
        })?;
        out.push(RaisedException {
            scoped_name: scoped_exception_name(ast, node)?,
            name: node.name.clone(),
        });
    }
    Ok(out)
}

/// How deep a module chain is followed before the model is taken to be
/// malformed. IDL nests nowhere near this.
const MAX_MODULE_DEPTH: usize = 64;

/// The `<interface_name>` of §7.4.1 for one interface: "concatenation of
/// module names separates by underscores followed by the name of the
/// interface".
fn qualified_name_of(ast: &Ast, iface: &InterfaceNode) -> RpcResult<String> {
    let mut chain: Vec<String> = Vec::new();
    let mut current = Some(iface.module);
    let mut depth = 0usize;
    while let Some(id) = current {
        if depth > MAX_MODULE_DEPTH {
            return Err(RpcError::StaleHandle { what: "module" });
        }
        depth += 1;
        let node = ast
            .module(id)
            .map_err(|_| RpcError::StaleHandle { what: "module" })?;
        if !node.name.is_empty() {
            chain.push(node.name.clone());
        }
        current = node.parent;
    }
    chain.reverse();
    let segments: Vec<&str> = chain.iter().map(String::as_str).collect();
    Ok(qualified_interface_name(&segments, &iface.name))
}

/// The scoped name of an exception, `::`-separated — what §7.5.1.1.5
/// rule 4b hashes.
fn scoped_exception_name(ast: &Ast, node: &zerodds_ast::ExceptionNode) -> RpcResult<String> {
    let mut chain: Vec<String> = Vec::new();
    let mut current = Some(node.module);
    let mut depth = 0usize;
    while let Some(id) = current {
        if depth > MAX_MODULE_DEPTH {
            return Err(RpcError::StaleHandle { what: "module" });
        }
        depth += 1;
        let module = ast
            .module(id)
            .map_err(|_| RpcError::StaleHandle { what: "module" })?;
        if !module.name.is_empty() {
            chain.push(module.name.clone());
        }
        current = module.parent;
    }
    chain.reverse();
    chain.push(node.name.clone());
    Ok(chain.join("::"))
}

fn lower_method(ast: &Ast, op: &OperationNode) -> RpcResult<MethodDef> {
    let name = op.name.clone();
    if name.is_empty() {
        return Err(RpcError::InvalidMethodName(name));
    }

    // Production (87) removed the `oneway` modifier, and §7.3.2 keeps
    // `@oneway` for Java source only; §7.5.1.1.7 rule 5 still requires a
    // Return-union case for every operation. Both spellings set
    // `OperationNode::oneway` in the frontend, so the flag is the whole
    // answer. `has_oneway` is consulted for a caller that lowered the
    // annotation somewhere the frontend did not fold.
    let method_anns = lower_rpc_annotations(ast, &op.facets);
    if op.oneway || method_anns.has_oneway() {
        return Err(RpcError::OutsideServiceGrammar {
            construct: "a `oneway` operation",
            at: name,
        });
    }

    // Production (87) also removed `context` expressions.
    if !op.contexts.is_empty() {
        return Err(RpcError::OutsideServiceGrammar {
            construct: "a `context` clause",
            at: name,
        });
    }

    let return_type = op.return_type.clone();

    let mut params = Vec::with_capacity(op.params.len());
    for p in &op.params {
        let p = ast
            .param(*p)
            .map_err(|_| RpcError::StaleHandle { what: "parameter" })?;
        // Param annotation `@in/@out/@inout` overrides the declared
        // direction, if explicitly set.
        let p_anns = lower_rpc_annotations(ast, &p.facets);
        let direction = override_direction(p.direction, &p_anns);

        params.push(ParamDef {
            name: p.name.clone(),
            direction,
            type_ref: p.type_.clone(),
        });
    }

    // Duplicate-Param-Check.
    for i in 0..params.len() {
        for j in (i + 1)..params.len() {
            if params[i].name == params[j].name {
                return Err(RpcError::DuplicateParam {
                    method: name,
                    param: params[i].name.clone(),
                });
            }
        }
    }

    let mut raises = Vec::with_capacity(op.raises.len());
    for raised in &op.raises {
        let id = raised.resolved().ok_or(RpcError::StaleHandle {
            what: "raised exception",
        })?;
        let node = ast.exception(id).map_err(|_| RpcError::StaleHandle {
            what: "raised exception",
        })?;
        raises.push(RaisedException {
            scoped_name: scoped_exception_name(ast, node)?,
            name: node.name.clone(),
        });
    }

    Ok(MethodDef {
        name,
        params,
        return_type,
        oneway: false,
        raises,
        mutable: method_anns.is_mutable(),
    })
}

fn override_direction(native: AstParamDirection, anns: &LoweredRpc) -> ParamDirection {
    use crate::annotations::RpcAnnotation;
    for a in &anns.builtins {
        match a {
            RpcAnnotation::In => return ParamDirection::In,
            RpcAnnotation::Out => return ParamDirection::Out,
            RpcAnnotation::InOut => return ParamDirection::InOut,
            _ => {}
        }
    }
    native.into()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use zerodds_ast::{InterfaceKind, PrimitiveKind, TypeKind};

    /// Parse, lower, resolve.
    ///
    /// The previous version of this module built `InterfaceDef` and
    /// `OpDecl` values by hand; the shapes it built were whatever this
    /// file believed the frontend produces. These lower real IDL, so a
    /// disagreement between the two shows up here.
    fn model(idl: &str) -> Ast {
        let parsed = zerodds_idl_parse::parse(idl);
        assert!(
            parsed.diagnostics().iter().all(|d| !d.is_error()),
            "the source does not parse: {:?}",
            parsed.diagnostics()
        );
        let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "rpc_service_mapping.idl");
        assert!(
            lowered.diagnostics.iter().all(|d| !d.is_error()),
            "the source does not lower: {:?}",
            lowered.diagnostics
        );
        let mut ast = lowered.ast;
        let report = ast.resolve_all_by_name().expect("a resolve pass");
        assert!(
            report.broken.is_empty(),
            "every name must resolve: {:?}",
            report.broken
        );
        ast
    }

    /// Lowers the one interface the source declares.
    fn service_of(idl: &str) -> RpcResult<ServiceDef> {
        let ast = model(idl);
        let (id, _) = ast.interfaces().next().expect("one interface");
        lower_service(&ast, id)
    }

    /// Lowers the interface the source declares last — the derived one,
    /// where the source declares a hierarchy.
    fn last_service_of(idl: &str) -> RpcResult<ServiceDef> {
        let ast = model(idl);
        let (id, _) = ast.interfaces().last().expect("an interface");
        lower_service(&ast, id)
    }

    #[test]
    fn calculator_service_with_in_params_lowers() {
        let svc = service_of("@service interface Calculator { long add(in long a, in long b); };")
            .unwrap();
        assert_eq!(svc.name, "Calculator");
        assert_eq!(svc.methods.len(), 1);
        let m = &svc.methods[0];
        assert_eq!(m.name, "add");
        assert!(!m.oneway);
        assert_eq!(m.params.len(), 2);
        assert_eq!(m.in_params().count(), 2);
        assert_eq!(m.out_params().count(), 0);
        assert!(m.return_type.is_some());
        assert_eq!(
            svc.topic_names().unwrap().request,
            "Calculator_Service_Request"
        );
    }

    #[test]
    fn the_oneway_keyword_is_outside_the_service_grammar() {
        // §7.3.1.1 production (87) removed the modifier; §7.5.1.1.7
        // rule 5 still requires a Return-union case for every operation,
        // so accepting a oneway operation changed the mapping.
        let err = service_of("interface Logger { oneway void log(in string msg); };").unwrap_err();
        assert!(matches!(
            err,
            RpcError::OutsideServiceGrammar {
                construct: "a `oneway` operation",
                ..
            }
        ));
    }

    #[test]
    fn the_oneway_annotation_is_outside_the_service_grammar() {
        // §7.3.2 keeps `@oneway` for Java source only; the Basic IDL
        // grammar has neither spelling.
        let err = service_of("interface Svc { @oneway void ping(); };").unwrap_err();
        assert!(matches!(
            err,
            RpcError::OutsideServiceGrammar {
                construct: "a `oneway` operation",
                ..
            }
        ));
    }

    #[test]
    fn a_context_clause_is_outside_the_service_grammar() {
        // §7.3.1.1 production (87) removed `context` expressions.
        let err = service_of("interface Svc { void ping() context (\"x\"); };").unwrap_err();
        assert!(matches!(
            err,
            RpcError::OutsideServiceGrammar {
                construct: "a `context` clause",
                ..
            }
        ));
    }

    #[test]
    fn a_nested_declaration_is_outside_the_service_grammar() {
        // §7.3.1.1 production (9) removed embedded declarations of types,
        // constants and exceptions from an interface body. A constant
        // used to be skipped silently.
        let err = service_of("interface Svc { const long MAX = 10; void foo(); };").unwrap_err();
        assert!(matches!(
            err,
            RpcError::OutsideServiceGrammar {
                construct: "a constant declaration in an interface body",
                ..
            }
        ));
    }

    #[test]
    fn a_nested_type_declaration_is_outside_the_service_grammar() {
        let err = service_of("interface Svc { struct S { long x; }; void foo(); };").unwrap_err();
        assert!(matches!(
            err,
            RpcError::OutsideServiceGrammar {
                construct: "a type declaration in an interface body",
                ..
            }
        ));
    }

    #[test]
    fn an_import_directive_is_outside_the_service_grammar() {
        // §7.3.1.1 production (1) removed the CORBA import statement.
        let err = service_of("import X; interface Svc { void foo(); };").unwrap_err();
        assert!(matches!(
            err,
            RpcError::OutsideServiceGrammar {
                construct: "an `import` directive",
                ..
            }
        ));
    }

    #[test]
    fn a_typeid_directive_is_outside_the_service_grammar() {
        // §7.3.1.1 production (2) removed `<type_id_dcl>` and
        // `<type_prefix_dcl>`.
        let err =
            service_of("typeid Svc \"IDL:Svc:1.0\"; interface Svc { void foo(); };").unwrap_err();
        assert!(matches!(
            err,
            RpcError::OutsideServiceGrammar {
                construct: "a `typeid`/`typeprefix` directive",
                ..
            }
        ));
    }

    #[test]
    fn a_typeprefix_directive_is_outside_the_service_grammar() {
        let err = service_of("typeprefix Svc \"IDL:example/\"; interface Svc { void foo(); };")
            .unwrap_err();
        assert!(matches!(
            err,
            RpcError::OutsideServiceGrammar {
                construct: "a `typeid`/`typeprefix` directive",
                ..
            }
        ));
    }

    #[test]
    fn a_component_declaration_is_outside_the_service_grammar() {
        // §7.3.1.1 production (2) removed `<component>`; production (7)
        // admits only the plain `interface` header.
        let err = service_of("component C { };").unwrap_err();
        assert!(matches!(
            err,
            RpcError::OutsideServiceGrammar {
                construct: "an interface declaration of a form other than `interface`",
                ..
            }
        ));
    }

    #[test]
    fn a_local_interface_is_outside_the_service_grammar() {
        let err = service_of("local interface Svc { void foo(); };").unwrap_err();
        assert!(matches!(
            err,
            RpcError::OutsideServiceGrammar {
                construct: "an interface declaration of a form other than `interface`",
                ..
            }
        ));
    }

    #[test]
    fn duplicate_method_detected() {
        // Two operations of the same name are a model the frontend
        // refuses to build, so this one is built through the model API.
        let mut ast = Ast::new();
        let root = ast.root_module();
        let iface = ast
            .add_interface(root, "Svc", InterfaceKind::Interface, Vec::new())
            .expect("an interface");
        ast.add_operation(iface, "foo", None, false)
            .expect("an operation");
        ast.add_operation(iface, "foo", None, false)
            .expect("a second operation");
        let err = lower_service(&ast, iface).unwrap_err();
        assert_eq!(err, RpcError::DuplicateMethod("foo".into()));
    }

    #[test]
    fn duplicate_param_detected() {
        let mut ast = Ast::new();
        let root = ast.root_module();
        let iface = ast
            .add_interface(root, "Svc", InterfaceKind::Interface, Vec::new())
            .expect("an interface");
        let long = ast
            .add_type(root, "Long", TypeKind::Primitive(PrimitiveKind::Int32))
            .expect("a type");
        let op = ast
            .add_operation(iface, "add", Some(Ref::Resolved(long)), false)
            .expect("an operation");
        ast.add_param(op, "x", AstParamDirection::In, Ref::Resolved(long))
            .expect("a parameter");
        ast.add_param(op, "x", AstParamDirection::In, Ref::Resolved(long))
            .expect("a second parameter");
        let err = lower_service(&ast, iface).unwrap_err();
        assert!(matches!(err, RpcError::DuplicateParam { .. }));
    }

    #[test]
    fn empty_method_name_rejected() {
        let mut ast = Ast::new();
        let root = ast.root_module();
        let iface = ast
            .add_interface(root, "Svc", InterfaceKind::Interface, Vec::new())
            .expect("an interface");
        ast.add_operation(iface, "", None, false)
            .expect("an operation");
        let err = lower_service(&ast, iface).unwrap_err();
        assert!(matches!(err, RpcError::InvalidMethodName(_)));
    }

    #[test]
    fn invalid_service_name_rejected() {
        // An IDL identifier cannot hold a hyphen, so the invalid name can
        // only arrive through the `@service(name = …)` override.
        let err = service_of(r#"@service(name = "Bad-Name") interface Svc { void ping(); };"#)
            .unwrap_err();
        assert!(matches!(err, RpcError::InvalidServiceName(_)));
    }

    /// §7.4.1: `@service(name = "…")` supplies `<service_name>`, which
    /// is the *middle* segment of the topic name. It does not rename the
    /// interface — `${interfaceName}` still builds every type name
    /// (§7.5.1.1.4-7).
    #[test]
    fn the_service_annotation_supplies_the_service_segment_only() {
        let svc =
            service_of(r#"@service(name = "OuterName") interface InternalIface { void ping(); };"#)
                .unwrap();
        assert_eq!(svc.name, "InternalIface");
        assert_eq!(svc.service_name, "OuterName");
        let topics = svc.topic_names().unwrap();
        assert_eq!(topics.request, "InternalIface_OuterName_Request");
        assert_eq!(topics.reply, "InternalIface_OuterName_Reply");
    }

    /// Without the annotation, `<service_name>` is `"Service"`.
    #[test]
    fn without_the_annotation_the_service_segment_is_the_default() {
        let svc = service_of("interface InternalIface { void ping(); };").unwrap();
        assert_eq!(svc.service_name, "Service");
        assert_eq!(
            svc.topic_names().unwrap().request,
            "InternalIface_Service_Request"
        );
    }

    /// §7.4.1: `<interface_name>` is "concatenation of module names
    /// separates by underscores followed by the name of the interface".
    #[test]
    fn the_module_chain_reaches_the_topic_name() {
        let svc = service_of("module robot { module arm { interface Joint { void park(); }; }; };")
            .unwrap();
        assert_eq!(svc.name, "Joint");
        assert_eq!(svc.qualified_name, "robot_arm_Joint");
        assert_eq!(
            svc.topic_names().unwrap().request,
            "robot_arm_Joint_Service_Request"
        );
    }

    /// §7.4.1's last paragraph: the request-reply binding does not
    /// capture the interface segment.
    #[test]
    fn the_request_reply_binding_drops_the_interface_segment() {
        let svc = service_of("module robot { interface Calculator { void ping(); }; };").unwrap();
        let names = svc
            .topic_names_for(crate::topic_naming::BindingStyle::RequestReply)
            .unwrap();
        assert_eq!(names.request, "Service_Request");
        assert_eq!(names.reply, "Service_Reply");
    }

    /// §7.4.3: each parent interface contributes a topic alias.
    #[test]
    fn every_base_interface_becomes_a_topic_alias() {
        let svc = last_service_of(
            "interface Adder { void a(); };
             interface Subtractor { void s(); };
             interface Calculator : Adder, Subtractor { void c(); };",
        )
        .unwrap();
        assert_eq!(svc.bases, ["Adder", "Subtractor"]);
        let topics = svc.topic_names().unwrap();
        assert_eq!(topics.request, "Calculator_Service_Request");
        assert_eq!(
            topics.request_aliases,
            ["Adder_Service_Request", "Subtractor_Service_Request"]
        );
    }

    /// §7.4.2.2: `@DDSRequestTopic` / `@DDSReplyTopic` give a whole
    /// topic name; §7.4.2.3 lets the run time beat them.
    #[test]
    fn the_topic_annotations_and_the_run_time_layer_have_their_order() {
        let svc = service_of(
            r#"@DDSRequestTopic(name = "MyReq") @DDSReplyTopic(name = "MyRep")
               interface Calculator { void ping(); };"#,
        )
        .unwrap();
        assert_eq!(svc.request_topic.as_deref(), Some("MyReq"));
        let annotated = svc.topic_names().unwrap();
        assert_eq!(annotated.request, "MyReq");
        assert_eq!(annotated.reply, "MyRep");

        let at_run_time = svc
            .topic_names_with(
                crate::topic_naming::BindingStyle::FunctionCall,
                &crate::topic_naming::TopicNameOverrides::with_service_name("Live"),
            )
            .unwrap();
        assert_eq!(at_run_time.request, "Calculator_Live_Request");
    }

    #[test]
    fn inout_param_appears_in_both_directions() {
        let svc = service_of("interface Svc { void swap(inout long v); };").unwrap();
        let m = &svc.methods[0];
        assert_eq!(m.in_params().count(), 1);
        assert_eq!(m.out_params().count(), 1);
    }

    #[test]
    fn out_only_param_is_reply_only() {
        let svc = service_of("interface Svc { void result(out long v); };").unwrap();
        let m = &svc.methods[0];
        assert_eq!(m.in_params().count(), 0);
        assert_eq!(m.out_params().count(), 1);
    }

    #[test]
    fn param_annotation_in_overrides_declared_direction() {
        // The parameter is declared `out` and annotated `@in`; the
        // annotation wins.
        //
        // `in` is an IDL keyword, so the annotation can only be written
        // with the leading-underscore escape — IDL 4.2 §7.2.3 — which
        // turns the keyword check off and leaves the identifier `in`.
        // Spelled `@in` the source does not parse at all.
        let svc = service_of(
            r"@annotation _in { boolean value default TRUE; };
              interface Svc { void foo(@_in out long v); };",
        )
        .unwrap();
        assert_eq!(svc.methods[0].params[0].direction, ParamDirection::In);
    }

    #[test]
    fn attributes_lower_into_the_implied_operations_of_seven_five_one_one_three() {
        // §7.5.1.1.3 rule 1: every attribute implies
        // get_attribute_<name>/set_attribute_<name> in the same
        // interface; `readonly` implies the getter alone. (A constant
        // cannot stand in the interface body — §7.3.1.1 production (9)
        // removed it.)
        let svc = service_of(
            "interface Svc { readonly attribute long level; attribute long state; void foo(); };",
        )
        .unwrap();
        assert_eq!(svc.methods.len(), 1);
        assert_eq!(svc.methods[0].name, "foo");
        assert_eq!(svc.attributes.len(), 2);
        let names: Vec<String> = svc.all_methods().iter().map(|m| m.name.clone()).collect();
        assert!(
            names.iter().any(|n| n == "get_attribute_level"),
            "{names:?}"
        );
        assert!(
            !names.iter().any(|n| n == "set_attribute_level"),
            "{names:?}"
        );
        assert!(
            names.iter().any(|n| n == "get_attribute_state"),
            "{names:?}"
        );
        assert!(
            names.iter().any(|n| n == "set_attribute_state"),
            "{names:?}"
        );
        assert!(names.iter().any(|n| n == "foo"), "{names:?}");
    }

    /// §7.5.1.1.3 rule 3: "The return type of the
    /// set_attribute_<attribute-name> operation is void and it accepts an
    /// argument of the same type as that of the attribute and the name of
    /// the argument is the same as the attribute name."
    #[test]
    fn the_implied_setter_takes_one_argument_named_like_the_attribute() {
        let svc = service_of("interface Svc { attribute long level; };").unwrap();
        let methods = svc.all_methods();
        let setter = methods
            .iter()
            .find(|m| m.name == "set_attribute_level")
            .expect("the setter");
        assert!(setter.return_type.is_none(), "void");
        assert_eq!(setter.params.len(), 1);
        assert_eq!(setter.params[0].name, "level");
        assert!(setter.params[0].direction.is_in());
        assert_eq!(setter.params[0].type_ref, svc.attributes[0].type_ref);
        let getter = methods
            .iter()
            .find(|m| m.name == "get_attribute_level")
            .expect("the getter");
        assert!(getter.params.is_empty(), "rule 2: no arguments");
    }

    /// §7.5.1.1.3 rules 4 and 5: the `getraises` list becomes the `raises`
    /// of `get_attribute_<name>`, the `setraises` list that of
    /// `set_attribute_<name>`.
    #[test]
    fn getraises_and_setraises_become_the_raises_of_the_implied_operations() {
        let svc = service_of(
            "exception NotReadable { long why; }; exception NotWritable { long why; };
             interface Svc {
                 attribute long level getraises (NotReadable) setraises (NotWritable);
             };",
        )
        .unwrap();
        let methods = svc.all_methods();
        let raised = |name: &str| -> Vec<String> {
            methods
                .iter()
                .find(|m| m.name == name)
                .expect("an implied operation")
                .raises
                .iter()
                .map(|r| r.name.clone())
                .collect()
        };
        assert_eq!(raised("get_attribute_level"), ["NotReadable"]);
        assert_eq!(raised("set_attribute_level"), ["NotWritable"]);
    }

    #[test]
    fn a_declared_operation_colliding_with_an_implied_one_is_refused() {
        // §7.5.1.1.3 rule 1: an interface may not carry both an attribute
        // `level` and a user-defined `get_attribute_level`.
        let err =
            service_of("interface Svc { attribute long level; long get_attribute_level(); };")
                .unwrap_err();
        assert!(matches!(err, RpcError::DuplicateMethod(name) if name == "get_attribute_level"));
    }

    #[test]
    fn a_stale_interface_handle_is_reported_rather_than_skipped() {
        let mut ast = Ast::new();
        let root = ast.root_module();
        let iface = ast
            .add_interface(root, "Svc", InterfaceKind::Interface, Vec::new())
            .expect("an interface");
        let other = Ast::new();
        let err = lower_service(&other, iface).unwrap_err();
        assert!(matches!(err, RpcError::StaleHandle { .. }));
    }

    #[test]
    fn param_direction_helpers() {
        assert!(ParamDirection::In.is_in());
        assert!(!ParamDirection::In.is_out());
        assert!(!ParamDirection::Out.is_in());
        assert!(ParamDirection::Out.is_out());
        assert!(ParamDirection::InOut.is_in());
        assert!(ParamDirection::InOut.is_out());
    }

    #[test]
    fn param_direction_from_the_model_direction() {
        assert_eq!(
            ParamDirection::from(AstParamDirection::In),
            ParamDirection::In
        );
        assert_eq!(
            ParamDirection::from(AstParamDirection::Out),
            ParamDirection::Out
        );
        assert_eq!(
            ParamDirection::from(AstParamDirection::InOut),
            ParamDirection::InOut
        );
    }
}
