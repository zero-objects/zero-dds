// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! An IDL `interface`, as the operations a language binding needs.
//!
//! The struct columns have a seam already: `adapter::cst::rust_items`,
//! `adapter::cpp::cpp_items` and `adapter::typescript::ts_items` each
//! walk `ast.types()` and turn one model node into one declaration. An
//! `interface` had no such walk. `adapter::ast` puts the node into the
//! graph and `rule::{cpp,java,typescript}` each carry an
//! `interface-to-…` rule that fires on it, and then nothing read the
//! result: `cpp_items` says so in as many words, and the other two
//! simply do not look. Every RPC language binding therefore had a rule
//! that matched and no output.
//!
//! This is the missing half, and it is deliberately language-neutral.
//! What a C++ binding needs from an `interface` — the operations, each
//! parameter's direction, the return type, the `raises` list, the
//! attributes and the inheritance chain — is what a Java binding needs
//! and what a TypeScript binding needs. Written three times it would be
//! three readings of OMG IDL 4.2 §7.4.3, and the columns would disagree
//! about which parameters land in a request before anyone noticed.
//!
//! ## What it is not
//!
//! Not the wire model. `crates/rpc/src/codegen.rs` derives the request
//! and reply topic types from DDS-RPC 1.0 §7.5.1 and is the authority on
//! their shape. This module states the *names* that mapping fixes —
//! [`request_type_name`] and the four beside it — so a generated header
//! and the runtime that has to read it agree, and
//! `crates/rpc/tests/language_binding_agreement.rs` holds the two
//! against each other rather than trusting that they were written from
//! the same paragraph.
//!
//! Not a dependency on `zerodds-rpc` either, which would be a cycle:
//! `zerodds-rpc` → `zerodds-dcps` → `zerodds-lang-rules`.
//!
//! ## Spec
//!
//! - OMG IDL 4.2 §7.4.3 — `<interface_dcl>`, `<op_dcl>`, `<attr_dcl>`.
//! - DDS-RPC 1.0 §7.5.1.1.3 — an attribute's implied operations.
//! - DDS-RPC 1.0 §7.5.1.1.4-8 — the request and reply topic types.
//! - DDS-RPC 1.0 §7.11 — the C++ and Java language bindings.
//! - DDS-TS 1.0 Ch11 — the TypeScript client/handler mapping.

use zerodds_ast::{
    Ast, AttributeNode, ExceptionNode, InterfaceKind, InterfaceNode, ModuleNode, OperationNode,
    ParamDirection, TypeNode,
};
use zerodds_ast_core::{Id, NodeId};

/// The member an `Out` structure carries an operation's return value
/// in. DDS-RPC 1.0 §7.5.1.1.5 rule 3b.
///
/// `return_` with the underscore trailing, which is what the clause
/// prints in its own non-normative example.
/// `crates/rpc/src/codegen.rs::method_out_members` writes the same name,
/// so a binding that spelled it differently would generate a reader for
/// a member the writer never wrote.
pub const RETURN_MEMBER: &str = "return_";

/// The member an `In` or `Out` structure carries when the operation
/// gives it nothing else. §7.5.1.1.4 rule 3c, §7.5.1.1.5 rule 3c.
pub const DUMMY_MEMBER: &str = "dummy";

/// The member the `default` case of a `Call` or `Return` union carries.
/// §7.5.1.1.6 rule 3.
pub const UNKNOWN_OP_MEMBER: &str = "unknownOp";

/// The member the `dds::RETCODE_OK` case of a `Result` union carries.
/// §7.5.1.1.5 rule 3a.
pub const RESULT_MEMBER: &str = "result";

/// The member a `Request` and a `Reply` structure carry their union in.
/// §7.5.1.1.6 rule 2b, §7.5.1.1.7 rule 3b.
pub const DATA_MEMBER: &str = "data";

/// The label of the `Result` union's success case. DDS-DCPS gives
/// `RETCODE_OK` the value 0.
pub const RETCODE_OK: i32 = 0;

/// The suffix a request topic and its topic type take. DDS-RPC 1.0
/// §7.4.1, §7.5.1.1.6.
pub const REQUEST_SUFFIX: &str = "_Request";

/// The suffix a reply topic and its topic type take. DDS-RPC 1.0 §7.4.1,
/// §7.5.1.1.7.
pub const REPLY_SUFFIX: &str = "_Reply";

/// The `<service_name>` a service takes when the IDL supplies none.
///
/// DDS-RPC 1.0 §7.4.1: "The `<service_name>` non-terminal may be
/// user-supplied. When it is not, it defaults to 'Service.'"
pub const DEFAULT_SERVICE_NAME: &str = "Service";

/// Why an interface did not reach the seam.
///
/// Four failures, each of which used to be a construct that reached no
/// output and said nothing about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InterfaceError {
    /// `interface I;` with no `interface I { … };` anywhere in the unit.
    ///
    /// DDS-TS 1.0 §11.7 makes this fatal and names it `DDS-TS-E002`.
    /// The C++ and Java bindings have the same hole for the same
    /// reason: a `Requester<I_Request, I_Reply>` over an interface whose
    /// operations never arrived is a class over two structs with no
    /// members.
    ForwardWithoutDefinition {
        /// The interface's IDL name.
        name: String,
    },
    /// The model lists a child and then will not hand it over.
    NoNode {
        /// What was asked for: `"operation"`, `"parameter"`,
        /// `"attribute"`, `"base interface"` or `"exception"`.
        kind: &'static str,
        /// Where it sits, as far as the model names it.
        at: String,
    },
    /// A reference the output has to name is not resolved.
    ///
    /// A return type, a parameter type, an attribute type, a base
    /// interface or a `raises` entry. Emitting the operation without it
    /// would produce a method with a parameter missing, which no
    /// compiler on the far side can tell from one the IDL declared that
    /// way.
    Unresolved {
        /// What does not resolve, in the source's terms.
        construct: &'static str,
        /// Where it sits, as far as the model names it.
        at: String,
    },
    /// The base chain is deeper than the walk follows.
    ///
    /// Stopping there quietly would drop the operations and attributes
    /// of every base past the limit from the flattened binding.
    TooDeep {
        /// The interface whose chain exceeded the limit.
        name: String,
        /// The limit.
        limit: usize,
    },
    /// A construct DDS-RPC 1.0 §7.3.1.1 removed from the service
    /// grammar: a `oneway` operation (production 87 and §7.5.1.1.7
    /// rule 5, which wants a Return-union case for every operation), a
    /// `context` clause, an interface form other than `interface`
    /// (production 7), an `import`/`typeid`/`typeprefix` directive
    /// (productions 1 and 2), or a type, constant or exception
    /// declaration in an interface body (production 9).
    OutsideServiceGrammar {
        /// The construct, in the spec's terms.
        construct: &'static str,
        /// Where it sits.
        at: String,
    },
    /// Two operations of one interface carry the same name. Covers the
    /// operations §7.5.1.1.3 rule 1 implies for an attribute: "It is
    /// illegal to have an interface with an attribute named
    /// `<attribute-name>` and user-defined operations named
    /// `get_attribute_<attribute-name>` and
    /// `set_attribute_<attribute-name>`."
    DuplicateOperation {
        /// The interface.
        interface: String,
        /// The colliding operation name.
        name: String,
    },
    /// Two parameters of one operation carry the same name.
    DuplicateParameter {
        /// `<interface>::<operation>`.
        at: String,
        /// The colliding parameter name.
        name: String,
    },
}

impl core::fmt::Display for InterfaceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ForwardWithoutDefinition { name } => write!(
                f,
                "interface `{name}` is forward-declared and never defined"
            ),
            Self::NoNode { kind, at } => {
                write!(f, "{at}: the model lists a {kind} it will not hand over")
            }
            Self::Unresolved { construct, at } => {
                write!(f, "{at}: the {construct} does not resolve")
            }
            Self::TooDeep { name, limit } => write!(
                f,
                "interface `{name}`: the inheritance chain is deeper than {limit} levels"
            ),
            Self::OutsideServiceGrammar { construct, at } => write!(
                f,
                "{at}: {construct} is outside the DDS-RPC 1.0 §7.3.1.1 service grammar"
            ),
            Self::DuplicateOperation { interface, name } => write!(
                f,
                "interface `{interface}`: the operation name `{name}` is used twice \
                 (DDS-RPC 1.0 §7.5.1.1.3 rule 1 covers the names an attribute implies)"
            ),
            Self::DuplicateParameter { at, name } => {
                write!(f, "{at}: the parameter name `{name}` is used twice")
            }
        }
    }
}

impl core::error::Error for InterfaceError {}

/// Wraps an interface-level failure as the one error type a
/// materialiser already returns, for a flattened-interface binding that
/// reads [`all_operations_flattened`]/[`all_attributes_flattened`]
/// straight from its own `interfaces` function and has no second error
/// type of its own to keep it in. `at` is the interface's own name —
/// the caller's loop already has it, and it is a better answer than
/// anything this function could derive from `error` alone.
#[must_use]
pub fn interface_error_to_materialise(
    error: InterfaceError,
    at: &str,
) -> crate::adapter::cst::MaterialiseError {
    crate::adapter::cst::MaterialiseError::Unevaluated {
        construct: format!("interface binding ({error})"),
        at: at.to_string(),
    }
}

/// Why an interface did not become a language binding.
///
/// Two halves, because two questions are being asked. [`InterfaceError`]
/// is about the interface: is it defined, do its references resolve.
/// [`crate::adapter::cst::MaterialiseError`] is about a type inside it:
/// does the target language have a form for a `sequence<any>`. A binding
/// that folded the two would report an unresolved parameter type and a
/// language with no spelling for `any` as the same failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingError {
    /// The interface did not reach the seam.
    Interface(InterfaceError),
    /// A type in it has no form in the target language.
    Materialise(crate::adapter::cst::MaterialiseError),
    /// A codec for the topic types of the interface could not be built,
    /// with the reason.
    Codec(String),
}

impl BindingError {
    /// The diagnostic code DDS-TS 1.0 Annex D gives this failure, if it
    /// gives one.
    ///
    /// Only §11.7's orphaned forward declaration has one: `DDS-TS-E002`,
    /// "forward-declared interface lacks complete declaration". Every
    /// other failure here is a refusal the code registry does not name.
    #[must_use]
    pub const fn diagnostic_code(&self) -> Option<&'static str> {
        match self {
            Self::Interface(InterfaceError::ForwardWithoutDefinition { .. }) => Some("DDS-TS-E002"),
            Self::Interface(_) | Self::Materialise(_) | Self::Codec(_) => None,
        }
    }
}

impl From<InterfaceError> for BindingError {
    fn from(error: InterfaceError) -> Self {
        Self::Interface(error)
    }
}

impl From<crate::adapter::cst::MaterialiseError> for BindingError {
    fn from(error: crate::adapter::cst::MaterialiseError) -> Self {
        Self::Materialise(error)
    }
}

impl core::fmt::Display for BindingError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Interface(error) => write!(f, "{error}"),
            Self::Materialise(error) => write!(f, "{error}"),
            Self::Codec(reason) => write!(f, "no codec for the topic types: {reason}"),
        }
    }
}

impl core::error::Error for BindingError {}

/// One entry of an operation's `raises` clause, or of an attribute's
/// `getraises` / `setraises`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raise {
    /// The exception's handle.
    pub id: Id<ExceptionNode>,
    /// Its simple IDL name, which §7.5.1.1.5 rule 5b lower-cases into
    /// the union member's.
    pub name: String,
    /// Its scoped name, `::`-separated — what §7.5.1.1.5 rule 4b
    /// hashes: "the HASH of the fully qualified name of the exception
    /// type where module separator to be used is `::` (2 colons)".
    pub scoped_name: String,
}

/// One parameter of an operation. OMG IDL 4.2 §7.4.3, rule (86).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    /// Its IDL name.
    pub name: String,
    /// `in`, `out` or `inout`.
    pub direction: ParamDirection,
    /// Its declared type.
    pub type_: Id<TypeNode>,
}

impl Parameter {
    /// Whether this parameter travels in the request. DDS-RPC 1.0
    /// §7.5.1.1.4: `in` and `inout`.
    #[must_use]
    pub const fn is_input(&self) -> bool {
        matches!(self.direction, ParamDirection::In | ParamDirection::InOut)
    }

    /// Whether this parameter travels in the reply. DDS-RPC 1.0
    /// §7.5.1.1.5: `out` and `inout`.
    #[must_use]
    pub const fn is_output(&self) -> bool {
        matches!(self.direction, ParamDirection::Out | ParamDirection::InOut)
    }
}

/// Where an operation came from.
///
/// An IDL `interface` declares operations directly and declares
/// attributes, and DDS-RPC 1.0 §7.5.1.1.3 turns each attribute into one
/// or two *implied* operations. The two are the same shape on the wire
/// and are not the same thing in a generated header — a C++ attribute
/// getter is spelled per DDS-Cxx-PSM §7.4.6 and a declared operation is
/// spelled per §7.11.1.1.2 rule 6 — so the origin travels with the
/// operation rather than being guessed from the name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Written as `<op_dcl>` in the IDL.
    Declared,
    /// The getter DDS-RPC 1.0 §7.5.1.1.3 implies for an attribute.
    AttributeGetter,
    /// The setter DDS-RPC 1.0 §7.5.1.1.3 implies for a writable
    /// attribute.
    AttributeSetter,
}

/// One operation of an interface. OMG IDL 4.2 §7.4.3, rule (82).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    /// Stable identity of the model node, absent for an implied
    /// operation, which has no `<op_dcl>` of its own.
    pub node_id: Option<NodeId>,
    /// Its IDL name, or the implied name of §7.5.1.1.3.
    pub name: String,
    /// The return type, absent for `void`.
    pub returns: Option<Id<TypeNode>>,
    /// Parameters in declaration order.
    pub params: Vec<Parameter>,
    /// The `raises` clause, in declaration order.
    pub raises: Vec<Raise>,
    /// Whether the IDL wrote it or §7.5.1.1.3 implied it.
    pub origin: Origin,
    /// Whether it carries `@Mutable` (DDS-RPC 1.0 §7.5.1.2.1.4): the
    /// Enhanced Service Mapping declares its `In`, `Out` and `Result`
    /// structures `@Extensibility(MUTABLE_EXTENSIBILITY)`. The Basic
    /// mapping ignores it.
    pub mutable: bool,
}

impl Operation {
    /// The attribute an implied accessor belongs to: the operation name
    /// without the `get_attribute_` / `set_attribute_` prefix of DDS-RPC 1.0
    /// §7.5.1.1.3 rule 1. For a declared operation, its own name.
    #[must_use]
    pub fn attribute_name(&self) -> &str {
        match self.origin {
            Origin::AttributeGetter => self.name.strip_prefix("get_attribute_"),
            Origin::AttributeSetter => self.name.strip_prefix("set_attribute_"),
            Origin::Declared => None,
        }
        .unwrap_or(&self.name)
    }

    /// The parameters a request carries, in declaration order.
    pub fn inputs(&self) -> impl Iterator<Item = &Parameter> {
        self.params.iter().filter(|param| param.is_input())
    }

    /// The parameters a reply carries, in declaration order.
    pub fn outputs(&self) -> impl Iterator<Item = &Parameter> {
        self.params.iter().filter(|param| param.is_output())
    }

    /// The `raises` clause, as one line of prose, or `None` where the
    /// operation raises nothing.
    ///
    /// DDS-RPC 1.0 §7.11.1.1.2 rule 6.c keeps the clause out of the C++
    /// signature — "The functions shall not have any exception
    /// specification even if the IDL operation has an exception
    /// specification" — and §7.11.2 does not override it for Java. It
    /// does not say to hide what the IDL declared, and a generated
    /// method with neither a `throws` clause nor a line naming the
    /// exceptions would be the one place in the pipeline where a
    /// declared `raises` reaches no output at all.
    #[must_use]
    pub fn raises_line(&self) -> Option<String> {
        raises_line(&self.raises)
    }

    /// Whether the reply carries anything beside the return value.
    ///
    /// DDS-RPC 1.0 §7.11.1.1.2 rule 7.i turns on this: an operation with
    /// `out` or `inout` parameters returns
    /// `future<${interface}_${operation}_Out>` and one without returns
    /// `future<T>` or `future<void>`.
    #[must_use]
    pub fn has_outputs(&self) -> bool {
        self.params.iter().any(Parameter::is_output)
    }
}

/// One attribute of an interface. OMG IDL 4.2 §7.4.3, rule (88).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    /// Stable identity of the model node.
    pub node_id: NodeId,
    /// Its IDL name.
    pub name: String,
    /// Its declared type.
    pub type_: Id<TypeNode>,
    /// Whether it is `readonly`.
    pub readonly: bool,
    /// The `getraises` clause, in declaration order.
    pub get_raises: Vec<Raise>,
    /// The `setraises` clause, in declaration order.
    pub set_raises: Vec<Raise>,
}

impl Attribute {
    /// [`raises_line`] over [`Self::get_raises`], for a language that
    /// renders the getter half of the attribute separately from the
    /// setter (a native property's `get` accessor, or a getter method).
    #[must_use]
    pub fn get_raises_line(&self) -> Option<String> {
        raises_line(&self.get_raises)
    }

    /// [`raises_line`] over [`Self::set_raises`].
    #[must_use]
    pub fn set_raises_line(&self) -> Option<String> {
        raises_line(&self.set_raises)
    }
}

/// A `raises`/`getraises`/`setraises` clause, as one line of prose, or
/// `None` where it is empty. [`Operation::raises_line`] and
/// [`Attribute::get_raises_line`]/[`Attribute::set_raises_line`] all
/// call this, so the wording is one decision and not three.
///
/// DDS-RPC 1.0 §7.11.1.1.2 rule 6.c keeps the clause out of the C++
/// signature — "The functions shall not have any exception
/// specification even if the IDL operation has an exception
/// specification" — and §7.11.2 does not override it for Java. It does
/// not say to hide what the IDL declared, and a generated member with
/// neither a `throws` clause nor a line naming the exceptions would be
/// the one place in the pipeline where a declared `raises` reaches no
/// output at all.
#[must_use]
pub fn raises_line(raises: &[Raise]) -> Option<String> {
    if raises.is_empty() {
        return None;
    }
    let names: Vec<&str> = raises.iter().map(|raise| raise.name.as_str()).collect();
    Some(format!(
        "Raises, as the IDL declares it: {}. DDS-RPC 1.0 §7.11.1.1.2 rule 6.c keeps it out of \
         the signature.",
        names.join(", ")
    ))
}

/// A base interface, as the inheritance specification names it. OMG IDL
/// 4.2 §7.4.3, rule (78).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Base {
    /// The base's handle.
    pub id: Id<InterfaceNode>,
    /// Its simple IDL name.
    pub name: String,
    /// Its `<interface_name>` of §7.4.1, which §7.4.3 turns into a
    /// topic alias.
    pub qualified_name: String,
}

/// One IDL `interface`, with everything a language binding reads off it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Service {
    /// The interface's handle.
    pub id: Id<InterfaceNode>,
    /// Stable identity of the model node.
    pub node_id: NodeId,
    /// Its IDL name.
    pub name: String,
    /// The module it is declared in.
    pub module: Id<ModuleNode>,
    /// The `<interface_name>` of DDS-RPC 1.0 §7.4.1: "concatenation of
    /// module names separates by underscores followed by the name of the
    /// interface".
    ///
    /// Distinct from [`Self::name`], and the distinction is a wire one:
    /// §7.5.1.1.6 builds the topic *type* names from the simple name
    /// (`Calculator_Request`) and §7.4.1 builds the *topic* names from
    /// this one (`robot_Calculator_Service_Request`). A binding that
    /// used one for both publishes a type nobody subscribes to.
    pub qualified_name: String,
    /// The `<service_name>` of §7.4.1 — what `@service(name = "…")`
    /// supplies, or [`DEFAULT_SERVICE_NAME`].
    pub service_name: String,
    /// `@DDSRequestTopic(name = "…")`, §7.4.2.2 — a whole topic name.
    /// "These annotations simply control the generated DDS wrapper
    /// code", so the generator is where they are read.
    pub request_topic: Option<String>,
    /// `@DDSReplyTopic(name = "…")`, §7.4.2.2.
    pub reply_topic: Option<String>,
    /// `@Qos(RequestProfile = "…")`, §7.10.1 — the URL of the XML QoS
    /// profile for the request endpoints.
    pub qos_request_profile: Option<String>,
    /// `@Qos(ReplyProfile = "…")`, §7.10.1 — the URL for the reply
    /// endpoints.
    pub qos_reply_profile: Option<String>,
    /// Which interface form the IDL wrote.
    pub kind: InterfaceKind,
    /// Base interfaces, in declaration order.
    pub bases: Vec<Base>,
    /// Operations, in declaration order.
    pub operations: Vec<Operation>,
    /// Attributes, in declaration order.
    pub attributes: Vec<Attribute>,
    /// The service mapping a binding is written for. [`service`] reads
    /// [`ServiceMapping::Basic`], the default; a generator that writes
    /// the Enhanced mapping sets it on the services it passes down, so
    /// that the pieces shared by the two mappings — the client and the
    /// dispatcher — know which of the two they bind.
    pub mapping: ServiceMapping,
}

impl Service {
    /// Whether this form has a remote invocation at all.
    ///
    /// A `local interface` (OMG IDL 4.2 §7.4.3.2) is by definition not
    /// invocable across a boundary, so a `Requester` over it would be a
    /// class whose whole purpose the declaration rules out. The abstract
    /// interface and the async interface are still emitted for it — it
    /// has operations, and a caller may hold one — which is why this is
    /// a question about the endpoints and not about the whole binding.
    ///
    /// The CCM forms (`component`, `home`, `connector`, `porttype`) have
    /// their own mapping in CCM 4.0 §6 and are not DDS-RPC services.
    #[must_use]
    pub const fn is_remote(&self) -> bool {
        matches!(
            self.kind,
            InterfaceKind::Interface | InterfaceKind::Abstract
        )
    }

    /// The request topic's name under `style`. DDS-RPC 1.0 §7.4.1, with
    /// §7.4.2.2's annotation where the IDL wrote one.
    ///
    /// The run-time layer of §7.4.2.3 is not here and cannot be: a
    /// generated header is written before there is a run time. What it
    /// carries is §7.4.2.1's default and §7.4.2.2's annotation, and the
    /// run time overrides both through
    /// `crates/rpc/src/topic_naming.rs::TopicNameOverrides`.
    #[must_use]
    pub fn request_topic_name(&self, style: BindingStyle) -> String {
        self.request_topic.clone().unwrap_or_else(|| {
            compose_topic_name(
                style,
                &self.qualified_name,
                &format!("{}{REQUEST_SUFFIX}", self.service_name),
            )
        })
    }

    /// The reply topic's name under `style`. See
    /// [`Self::request_topic_name`].
    #[must_use]
    pub fn reply_topic_name(&self, style: BindingStyle) -> String {
        self.reply_topic.clone().unwrap_or_else(|| {
            compose_topic_name(
                style,
                &self.qualified_name,
                &format!("{}{REPLY_SUFFIX}", self.service_name),
            )
        })
    }

    /// §7.4.3's topic aliases: one per parent interface, same tail.
    ///
    /// Empty where the topic name came from an annotation — a whole name
    /// has no interface segment to vary — and empty under the
    /// request-reply binding, where the name carries none either.
    #[must_use]
    pub fn topic_aliases(&self, style: BindingStyle, reply: bool) -> Vec<String> {
        let annotated = if reply {
            self.reply_topic.is_some()
        } else {
            self.request_topic.is_some()
        };
        if annotated || !style.captures_interface_name() {
            return Vec::new();
        }
        let suffix = format!(
            "{}{}",
            self.service_name,
            if reply { REPLY_SUFFIX } else { REQUEST_SUFFIX }
        );
        self.bases
            .iter()
            .map(|base| compose_topic_name(style, &base.qualified_name, &suffix))
            .collect()
    }

    /// Every operation a binding emits: the declared ones, then the ones
    /// DDS-RPC 1.0 §7.5.1.1.3 implies from the attributes.
    ///
    /// Rule 1: "Each attribute in an interface maps to a pair of IDL
    /// operations `get_attribute_<attribute-name>` and
    /// `set_attribute_<attribute-name>` in the same interface." The
    /// order is declaration order of the operations followed by
    /// declaration order of the attributes, which is what fixes the
    /// discriminator values of the call union: a generator that sorted
    /// them would renumber a service that had only gained an attribute.
    #[must_use]
    pub fn all_operations(&self) -> Vec<Operation> {
        let mut out = self.operations.clone();
        for attribute in &self.attributes {
            out.push(Operation {
                node_id: Some(attribute.node_id),
                name: format!("get_attribute_{}", attribute.name),
                returns: Some(attribute.type_),
                params: Vec::new(),
                raises: attribute.get_raises.clone(),
                origin: Origin::AttributeGetter,
                mutable: false,
            });
            if attribute.readonly {
                continue;
            }
            out.push(Operation {
                node_id: Some(attribute.node_id),
                name: format!("set_attribute_{}", attribute.name),
                returns: None,
                params: vec![Parameter {
                    name: attribute.name.clone(),
                    direction: ParamDirection::In,
                    type_: attribute.type_,
                }],
                raises: attribute.set_raises.clone(),
                origin: Origin::AttributeSetter,
                mutable: false,
            });
        }
        out
    }
}

/// The request topic type's name. DDS-RPC 1.0 §7.5.1.1.6 rule 1: "The
/// name of Request structure shall be `${interfaceName}_Request` in the
/// same module as the interface" — the *simple* interface name, and not
/// the topic name, which §7.4.1 builds differently.
#[must_use]
pub fn request_type_name(service: &str) -> String {
    format!("{service}{REQUEST_SUFFIX}")
}

/// The reply topic type's name. DDS-RPC 1.0 §7.5.1.1.7.
#[must_use]
pub fn reply_type_name(service: &str) -> String {
    format!("{service}{REPLY_SUFFIX}")
}

/// Which language binding a topic name is being built for. DDS-RPC 1.0
/// §7.4.1, last paragraph: "For the request-reply style language
/// binding, `<interface_name>` and the following underscore shall not be
/// captured automatically."
///
/// The same value `crates/rpc/src/topic_naming.rs::BindingStyle` carries
/// for the run time. Two enums and not one because this crate must not
/// depend on `zerodds-rpc` — that edge is a cycle, `zerodds-rpc` →
/// `zerodds-dcps` → `zerodds-lang-rules` — and
/// `crates/rpc/tests/language_binding_agreement.rs` holds the names the
/// two produce against each other rather than trusting the duplication.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BindingStyle {
    /// §7.11.1.1.2 — the interface segment is captured.
    #[default]
    FunctionCall,
    /// §7.11.1.1.1 — it is not.
    RequestReply,
}

impl BindingStyle {
    /// Both styles.
    pub const ALL: [Self; 2] = [Self::FunctionCall, Self::RequestReply];

    /// Whether this style prefixes `<interface_name>` and an underscore.
    #[must_use]
    pub const fn captures_interface_name(self) -> bool {
        matches!(self, Self::FunctionCall)
    }
}

/// The `<interface_name>` of §7.4.1 from a module chain and a simple
/// name: the module names joined by underscores, then the interface.
#[must_use]
pub fn qualified_interface_name(modules: &[&str], interface: &str) -> String {
    let mut out = String::new();
    for module in modules {
        if module.is_empty() {
            continue;
        }
        out.push_str(module);
        out.push('_');
    }
    out.push_str(interface);
    out
}

/// One topic name of §7.4.1, from the interface segment and the tail
/// after it.
#[must_use]
pub fn compose_topic_name(style: BindingStyle, qualified_interface: &str, suffix: &str) -> String {
    if style.captures_interface_name() {
        format!("{qualified_interface}_{suffix}")
    } else {
        suffix.to_string()
    }
}

/// The request union's name. DDS-RPC 1.0 §7.5.1.1.4.
#[must_use]
pub fn call_union_name(service: &str) -> String {
    format!("{service}_Call")
}

/// The reply union's name. DDS-RPC 1.0 §7.5.1.1.7 rule 1: "The Return
/// union name shall be `${interfaceName}_Return`."
///
/// Not `${I}_Result` — that name belongs to the *per-operation* union of
/// §7.5.1.1.5, and one name over two shapes is how a reader ends up
/// decoding the wrong one.
#[must_use]
pub fn return_union_name(service: &str) -> String {
    format!("{service}_Return")
}

/// The per-operation result union's name. DDS-RPC 1.0 §7.5.1.1.5 rule
/// 1.
#[must_use]
pub fn result_union_name(service: &str, operation: &str) -> String {
    format!("{service}_{operation}_Result")
}

/// The constant §7.5.1.1.6 rule 4 requires beside a `Call` union:
/// `${interfaceName}_${operationName}_Hash`.
#[must_use]
pub fn operation_hash_constant(service: &str, operation: &str) -> String {
    format!("{service}_{operation}_Hash")
}

/// The constant §7.5.1.1.5 rule 4a requires beside a `Result` union:
/// `${exceptionType}_Ex_Hash`.
#[must_use]
pub fn exception_hash_constant(exception: &str) -> String {
    format!("{exception}_Ex_Hash")
}

/// The member name §7.5.1.1.5 rule 5b gives an exception case: "the
/// lower-case version of the exception name with the suffix `_ex`
/// added".
#[must_use]
pub fn exception_member_name(exception: &str) -> String {
    format!("{}_ex", exception.to_lowercase())
}

/// The per-operation request struct's name. DDS-RPC 1.0 §7.5.1.1.4.
#[must_use]
pub fn in_struct_name(service: &str, operation: &str) -> String {
    format!("{service}_{operation}_In")
}

/// The per-operation reply struct's name.
///
/// DDS-RPC 1.0 §7.5.1.1.5 names it as the reply case type, and
/// §7.11.1.1.2 rule 7.i names the same type as what an asynchronous
/// operation with `out` or `inout` parameters returns a `future` of.
#[must_use]
pub fn out_struct_name(service: &str, operation: &str) -> String {
    format!("{service}_{operation}_Out")
}

/// The discriminator label a `Call` or `Return` union gives an
/// operation. DDS-RPC 1.0 §7.5.1.1.6 rule 5a: "The integral value of the
/// case label shall be the HASH of `${operationName}`" — the unqualified
/// name, through the algorithm of §7.5.1.1.2.
///
/// The same 32 bits `crates/rpc/src/rpc_hash.rs::rpc_member_hash`
/// produces, read as the `long` the union's discriminator is.
/// `crates/rpc/tests/language_binding_agreement.rs` holds the two
/// against each other.
#[must_use]
pub fn operation_hash(operation: &str) -> i32 {
    // MD5 of the name, first four octets, little-endian — §7.5.1.1.2's
    // pseudo-implementation reads them as
    // `md5[0] + 256*md5[1] + 256*256*md5[2] + 256*256*256*md5[3]`.
    let digest = zerodds_types::hash::hash_bytes(operation.as_bytes());
    u32::from_le_bytes([digest.0[0], digest.0[1], digest.0[2], digest.0[3]]) as i32
}

/// The HASH of §7.5.1.1.5 rule 4b: the exception's *fully qualified*
/// name, `::`-separated.
#[must_use]
pub fn exception_hash(scoped_name: &str) -> i32 {
    operation_hash(scoped_name)
}

/// How deep a module chain is followed before the model is taken to be
/// malformed. IDL nests nowhere near this.
pub const MAX_MODULE_DEPTH: usize = 64;

/// The modules a declaration nests in, outermost first, with the handle
/// and the IDL name of each.
///
/// The root module is the anonymous global scope and contributes no
/// entry. Every column already has this walk — `adapter::cpp`,
/// `adapter::java` and `adapter::typescript` each keep a private copy,
/// each escaping to its own alphabet — and a fourth copy escaping to a
/// fourth would be a fourth answer to the question of where a
/// declaration goes. This one escapes to none: the caller spells the
/// names, because that is the only part of the answer that is the
/// language's.
///
/// # Errors
/// [`InterfaceError::NoNode`] when the model does not answer for a
/// module in the chain, or the chain runs past [`MAX_MODULE_DEPTH`].
pub fn module_chain(
    ast: &Ast,
    module: Id<ModuleNode>,
) -> Result<Vec<(Id<ModuleNode>, String)>, InterfaceError> {
    let mut out = Vec::new();
    let mut current = Some(module);
    let mut depth = 0usize;
    while let Some(id) = current {
        if depth > MAX_MODULE_DEPTH {
            return Err(InterfaceError::NoNode {
                kind: "module",
                at: format!("the parent chain runs past {MAX_MODULE_DEPTH}"),
            });
        }
        depth += 1;
        let node = ast.module(id).map_err(|_| InterfaceError::NoNode {
            kind: "module",
            at: "the enclosing module".to_string(),
        })?;
        if !node.name.is_empty() {
            out.push((id, node.name.clone()));
        }
        current = node.parent;
    }
    out.reverse();
    Ok(out)
}

/// Every interface of the model, in the order the model iterates them.
///
/// # Errors
/// [`InterfaceError`], naming the interface and what about it did not
/// reach the seam — never a silently dropped operation, parameter or
/// attribute.
pub fn services(ast: &Ast) -> Result<Vec<Service>, InterfaceError> {
    let mut out = Vec::new();
    for (id, _) in ast.interfaces() {
        out.push(service(ast, id)?);
    }
    Ok(out)
}

/// One interface of the model.
///
/// # Errors
/// As [`services`].
pub fn service(ast: &Ast, id: Id<InterfaceNode>) -> Result<Service, InterfaceError> {
    let node = ast.interface(id).map_err(|_| InterfaceError::NoNode {
        kind: "interface",
        at: "the model".to_string(),
    })?;
    if node.completeness != zerodds_ast::Completeness::Complete {
        return Err(InterfaceError::ForwardWithoutDefinition {
            name: node.name.clone(),
        });
    }

    reject_outside_service_grammar(ast, node)?;

    let mut bases = Vec::with_capacity(node.bases.len());
    for base in &node.bases {
        let base_id = base.resolved().ok_or_else(|| InterfaceError::Unresolved {
            construct: "base interface",
            at: node.name.clone(),
        })?;
        let base_node = ast.interface(base_id).map_err(|_| InterfaceError::NoNode {
            kind: "base interface",
            at: node.name.clone(),
        })?;
        bases.push(Base {
            id: base_id,
            name: base_node.name.clone(),
            qualified_name: qualified_name_of(ast, base_node)?,
        });
    }

    let mut operations = Vec::with_capacity(node.operations.len());
    for operation in &node.operations {
        let op = ast
            .operation(*operation)
            .map_err(|_| InterfaceError::NoNode {
                kind: "operation",
                at: node.name.clone(),
            })?;
        operations.push(one_operation(ast, &node.name, op)?);
    }

    let mut attributes = Vec::with_capacity(node.attributes.len());
    for attribute in &node.attributes {
        let attr = ast
            .attribute(*attribute)
            .map_err(|_| InterfaceError::NoNode {
                kind: "attribute",
                at: node.name.clone(),
            })?;
        attributes.push(one_attribute(ast, &node.name, attr)?);
    }

    let service = Service {
        id,
        node_id: node.node_id,
        qualified_name: qualified_name_of(ast, node)?,
        service_name: service_annotation_name(ast, &node.facets)
            .unwrap_or_else(|| DEFAULT_SERVICE_NAME.to_string()),
        request_topic: annotation_string(ast, &node.facets, "DDSRequestTopic", "name"),
        reply_topic: annotation_string(ast, &node.facets, "DDSReplyTopic", "name"),
        qos_request_profile: annotation_named_string(ast, &node.facets, "Qos", "RequestProfile"),
        qos_reply_profile: annotation_named_string(ast, &node.facets, "Qos", "ReplyProfile"),
        name: node.name.clone(),
        module: node.module,
        kind: node.kind,
        bases,
        operations,
        attributes,
        mapping: ServiceMapping::Basic,
    };

    // §7.5.1.1.3 rule 1, and the plain duplicate: the declared
    // operations and the ones the attributes imply share one namespace.
    let every = service.all_operations();
    for (index, operation) in every.iter().enumerate() {
        if every[..index]
            .iter()
            .any(|other| other.name == operation.name)
        {
            return Err(InterfaceError::DuplicateOperation {
                interface: service.name.clone(),
                name: operation.name.clone(),
            });
        }
    }
    Ok(service)
}

/// DDS-RPC 1.0 §7.3.1.1: what the service grammar removed from IDL 4.2.
/// The same list `zerodds_rpc::service_mapping::lower_service` refuses;
/// `zerodds-rpc` cannot be a dependency of this crate (it would be a
/// cycle), so the check is written here against the same model.
fn reject_outside_service_grammar(ast: &Ast, iface: &InterfaceNode) -> Result<(), InterfaceError> {
    let refuse = |construct: &'static str, at: String| {
        Err(InterfaceError::OutsideServiceGrammar { construct, at })
    };
    // Production (7): only `interface` is in `<interface_header>`.
    if iface.kind != InterfaceKind::Interface {
        return refuse(
            "an interface declaration of a form other than `interface`",
            format!("{} {}", iface.kind.as_str(), iface.name),
        );
    }
    // Productions (1) and (2): file-scoped directives are kept on the
    // root module because they leave no node of their own.
    if let Ok(module) = ast.module(ast.root_module()) {
        if let Some(corba) = module.facets.corba.as_ref() {
            if let Some(first) = corba.imports.first() {
                return refuse("an `import` directive", first.clone());
            }
            if let Some(first) = corba.repository_directives.first() {
                return refuse("a `typeid`/`typeprefix` directive", first.clone());
            }
        }
    }
    // Production (9): an interface body holds attributes and operations
    // only. A nested declaration is lowered into the model all the same,
    // so it is found by its declaration scope.
    let nested = |module: Id<ModuleNode>, scope: Option<&str>| {
        module == iface.module && scope == Some(iface.name.as_str())
    };
    for (_, node) in ast.types() {
        if nested(node.module, node.scope.as_deref()) {
            return refuse(
                "a type declaration in an interface body",
                format!("{}::{}", iface.name, node.name),
            );
        }
    }
    for (_, node) in ast.constants() {
        if nested(node.module, node.scope.as_deref()) {
            return refuse(
                "a constant declaration in an interface body",
                format!("{}::{}", iface.name, node.name),
            );
        }
    }
    for (_, node) in ast.exceptions() {
        if nested(node.module, node.scope.as_deref()) {
            return refuse(
                "an exception declaration in an interface body",
                format!("{}::{}", iface.name, node.name),
            );
        }
    }
    Ok(())
}

/// Same bound `Ast::all_operations_of`'s own walk uses, so a cycle this
/// crate refuses is refused at the same depth the frontend already
/// refuses it at.
const MAX_INHERITANCE_DEPTH: usize = 64;

/// Every operation a *flattened* interface binding writes:
/// [`Service::all_operations`] — this interface's own declared
/// operations, plus the ones DDS-RPC 1.0 §7.5.1.1.3 implies from its
/// attributes — preceded by the same from every base, recursively, in
/// the order [`Ast::all_operations_of`] already walks bases: depth- and
/// cycle-bounded by `MAX_INHERITANCE_DEPTH`, and deduplicated by node
/// identity where a diamond makes one operation or attribute reachable
/// through more than one path.
///
/// OMG IDL 4.2 §7.4.3: a derived interface "inherits all the operations
/// and attributes of its base interfaces" — `Ast::all_operations_of`
/// gives the first half; this is both, for the language bindings with
/// no native multiple-supertrait mechanism to compose them through
/// separately instead. `emit::interface_rust` has one and reaches for
/// it rather than this.
///
/// # Errors
/// As [`service`], for any interface reached along the chain.
pub fn all_operations_flattened(
    ast: &Ast,
    interface: Id<InterfaceNode>,
) -> Result<Vec<Operation>, InterfaceError> {
    let mut seen_interfaces = Vec::new();
    let mut seen_ops = Vec::new();
    let mut out = Vec::new();
    collect_flattened_operations(
        ast,
        interface,
        &mut seen_interfaces,
        &mut seen_ops,
        &mut out,
        0,
    )?;
    Ok(out)
}

/// Every attribute a *flattened* interface binding writes as a native
/// property (C#, Swift, …, rather than a `get_`/`set_` method pair):
/// this interface's own attributes, preceded by every base's, in the
/// same order and under the same bound as
/// [`all_operations_flattened`].
///
/// A binding that renders these as properties calls this instead of
/// [`all_operations_flattened`] for the attribute half and filters that
/// one to [`Origin::Declared`] for the operation half — the two
/// together are §7.4.3's "all the operations and attributes", once
/// each, never both ways for the same attribute.
///
/// # Errors
/// As [`service`], for any interface reached along the chain.
pub fn all_attributes_flattened(
    ast: &Ast,
    interface: Id<InterfaceNode>,
) -> Result<Vec<Attribute>, InterfaceError> {
    let mut seen_interfaces = Vec::new();
    let mut seen_attrs = Vec::new();
    let mut out = Vec::new();
    collect_flattened_attributes(
        ast,
        interface,
        &mut seen_interfaces,
        &mut seen_attrs,
        &mut out,
        0,
    )?;
    Ok(out)
}

/// zerodds-lint: recursion-depth 64 (one level per base interface;
/// `depth` refuses a chain at `MAX_INHERITANCE_DEPTH`, and
/// `seen_interfaces` admits each interface once, so a cycle stops on the
/// second visit)
fn collect_flattened_attributes(
    ast: &Ast,
    interface: Id<InterfaceNode>,
    seen_interfaces: &mut Vec<Id<InterfaceNode>>,
    seen_attrs: &mut Vec<NodeId>,
    out: &mut Vec<Attribute>,
    depth: usize,
) -> Result<(), InterfaceError> {
    // A base reached twice (a diamond) is already in `out`; a chain past
    // the limit is refused rather than cut.
    if seen_interfaces.contains(&interface) {
        return Ok(());
    }
    let this = service(ast, interface)?;
    if depth >= MAX_INHERITANCE_DEPTH {
        return Err(InterfaceError::TooDeep {
            name: this.name.clone(),
            limit: MAX_INHERITANCE_DEPTH,
        });
    }
    seen_interfaces.push(interface);
    for base in &this.bases {
        collect_flattened_attributes(ast, base.id, seen_interfaces, seen_attrs, out, depth + 1)?;
    }
    for attribute in this.attributes {
        if seen_attrs.contains(&attribute.node_id) {
            continue;
        }
        seen_attrs.push(attribute.node_id);
        out.push(attribute);
    }
    Ok(())
}

/// zerodds-lint: recursion-depth 64 (one level per base interface;
/// `depth` refuses a chain at `MAX_INHERITANCE_DEPTH`, and
/// `seen_interfaces` admits each interface once, so a cycle stops on the
/// second visit)
fn collect_flattened_operations(
    ast: &Ast,
    interface: Id<InterfaceNode>,
    seen_interfaces: &mut Vec<Id<InterfaceNode>>,
    seen_ops: &mut Vec<(NodeId, String)>,
    out: &mut Vec<Operation>,
    depth: usize,
) -> Result<(), InterfaceError> {
    // A base reached twice (a diamond) is already in `out`; a chain past
    // the limit is refused rather than cut.
    if seen_interfaces.contains(&interface) {
        return Ok(());
    }
    let this = service(ast, interface)?;
    if depth >= MAX_INHERITANCE_DEPTH {
        return Err(InterfaceError::TooDeep {
            name: this.name.clone(),
            limit: MAX_INHERITANCE_DEPTH,
        });
    }
    seen_interfaces.push(interface);
    for base in &this.bases {
        collect_flattened_operations(ast, base.id, seen_interfaces, seen_ops, out, depth + 1)?;
    }
    for operation in this.all_operations() {
        if let Some(id) = operation.node_id {
            // `(node_id, name)`, not `node_id` alone: an attribute's
            // getter and setter (`Service::all_operations`) share one
            // `node_id` — both point back at the one `attribute`
            // declaration that implies them — so `node_id` alone would
            // make the setter look like a diamond-repeat of the getter
            // and drop it. A diamond repeat (the same operation reached
            // through two base paths) shares both the `node_id` *and*
            // the name; a getter/setter pair shares only the former.
            let key = (id, operation.name.clone());
            if seen_ops.contains(&key) {
                continue;
            }
            seen_ops.push(key);
        }
        out.push(operation);
    }
    Ok(())
}

// -- the Enhanced Service Mapping -------------------------------------------

/// Which of the two service mappings of DDS-RPC 1.0 §7.5.1 a generator
/// writes the topic types of.
///
/// The Basic mapping is the default: §7.5.1.1 is the one every DDS
/// implementation can carry, and the Enhanced one (§7.5.1.2) needs
/// XTypes. An interface does not say which of the two it is for — the
/// same IDL has both mappings — so the choice is the generator's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ServiceMapping {
    /// §7.5.1.1: a `Request` structure with a header and a `Call`
    /// union, a `Reply` structure with a header and a `Return` union.
    #[default]
    Basic,
    /// §7.5.1.2: `@Choice @Autoid` `Request` and `Reply` structures
    /// over the operations, without a header. An operation with nothing to
    /// carry has the `dummy` member of §7.5.1.1.4 rule 3c, as the example
    /// of §7.5.1.2.6 shows ([`EmptyOperationStyle::DummyMember`]).
    Enhanced,
    /// [`Self::Enhanced`], with an operation that has nothing to carry as
    /// an empty structure marked `@Empty` (§7.5.1.2.1.3,
    /// [`EmptyOperationStyle::EmptyAnnotation`]): the other reading of the
    /// clause.
    EnhancedEmptyStructures,
}

impl ServiceMapping {
    /// Whether this is one of the Enhanced readings.
    #[must_use]
    pub const fn is_enhanced(self) -> bool {
        matches!(self, Self::Enhanced | Self::EnhancedEmptyStructures)
    }

    /// How an operation with nothing to carry is mapped.
    #[must_use]
    pub const fn empty_style(self) -> EmptyOperationStyle {
        match self {
            Self::EnhancedEmptyStructures => EmptyOperationStyle::EmptyAnnotation,
            Self::Basic | Self::Enhanced => EmptyOperationStyle::DummyMember,
        }
    }
}

/// The member a `Reply` structure carries the remote exception code in.
/// §7.5.1.2.5 rule 5.
pub const REMOTE_EX_MEMBER: &str = "remoteEx";

/// The mask that turns the 32-bit HASH of §7.5.1.1.2 into a member id,
/// which has 28 bits (XTypes 1.3 §7.4.3.4.2, EMHEADER).
pub const MEMBER_ID_MASK: u32 = 0x0FFF_FFFF;

/// How an operation with nothing to carry is mapped in the Enhanced
/// Service Mapping.
///
/// §7.5.1.2.1.3 introduces `@Empty` for it, while §7.5.1.2.2 and
/// §7.5.1.2.3 take over the `dummy` member of §7.5.1.1.4 rule 3c and
/// the example of §7.5.1.2.6 shows it. The default is the `dummy`
/// member. The same setting `crates/rpc/src/codegen.rs` has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EmptyOperationStyle {
    /// One `dummy` member of type `UnusedMember`.
    #[default]
    DummyMember,
    /// An empty structure marked `@Empty`.
    EmptyAnnotation,
}

/// The type of a member of an Enhanced structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnhancedMemberType {
    /// A parameter's or the return value's IDL type.
    Idl(Id<TypeNode>),
    /// `dds::rpc::UnusedMember`, the `dummy` member.
    Dummy,
    /// A structure of this mapping, by name.
    Struct(String),
    /// An exception the operation raises (§7.5.1.2.3 rule 6b).
    Exception(Raise),
    /// `dds::rpc::RemoteExceptionCode_t`, the type of `remoteEx`. The
    /// clause prints `dds::rpc::RemoteException_t`, which no clause
    /// defines.
    RemoteException,
}

/// One member of an Enhanced structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnhancedMember {
    /// The member's name.
    pub name: String,
    /// Its type.
    pub type_: EnhancedMemberType,
    /// The member id `@Autoid` gives it: the HASH of the name
    /// (§7.5.1.2.1.2), 28 bits of it.
    pub member_id: u32,
}

/// A structure of the Enhanced Service Mapping.
///
/// The same shape as `crates/rpc/src/codegen.rs::EnhancedStruct`;
/// `crates/rpc/tests/language_binding_agreement.rs` holds the two against
/// each other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnhancedStruct {
    /// The structure's name.
    pub name: String,
    /// `@Choice` (§7.5.1.2.1.1).
    pub choice: bool,
    /// `@Autoid` (§7.5.1.2.1.2).
    pub autoid: bool,
    /// `@Empty` (§7.5.1.2.1.3).
    pub empty: bool,
    /// `@Extensibility(MUTABLE_EXTENSIBILITY)`.
    pub mutable: bool,
    /// The members, in the order the clause lists them.
    pub members: Vec<EnhancedMember>,
}

/// One operation's three structures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnhancedOperation {
    /// The simple name of the interface that declares the operation,
    /// which the structure names are built from: an inherited operation
    /// keeps its base's name (§7.5.1.2.6).
    pub interface: String,
    /// The module that interface is declared in, where its structures
    /// are.
    pub module: Id<ModuleNode>,
    /// The operation.
    pub operation: Operation,
    /// `${I}_${op}_In` (§7.5.1.2.2).
    pub in_struct: EnhancedStruct,
    /// `${I}_${op}_Out` (§7.5.1.2.3 rule 1).
    pub out_struct: EnhancedStruct,
    /// `${I}_${op}_Result` (§7.5.1.2.3 rules 2 to 7).
    pub result_struct: EnhancedStruct,
}

/// An interface under the Enhanced Service Mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnhancedService {
    /// Every operation: the inherited ones, base by base, and then the
    /// interface's own (the order of the example of §7.5.1.2.6).
    pub operations: Vec<EnhancedOperation>,
    /// `${interfaceName}_Request` (§7.5.1.2.4).
    pub request: EnhancedStruct,
    /// `${interfaceName}_Reply` (§7.5.1.2.5).
    pub reply: EnhancedStruct,
}

/// An operation with the interface that declares it and that interface's
/// module.
pub type DeclaredOperation = (String, Id<ModuleNode>, Operation);

/// The member id `@Autoid` gives a member of this name.
#[must_use]
pub fn member_id(name: &str) -> u32 {
    (operation_hash(name) as u32) & MEMBER_ID_MASK
}

/// The operations of `service` and of every base, each with the simple
/// name of the interface that declares it and the module it is in.
///
/// The bases in declaration order, each base's own bases before the base
/// and the service's own operations last — the order of the example of
/// §7.5.1.2.6, in which `Calculator_Request` lists `add` and `sub` of
/// its bases before `on` and `off` — a base reached twice listed once.
///
/// # Errors
/// As [`service`], for any interface reached along the chain.
pub fn operations_with_interface_flattened(
    ast: &Ast,
    service: &Service,
) -> Result<Vec<DeclaredOperation>, InterfaceError> {
    let mut out: Vec<DeclaredOperation> = Vec::new();
    let mut seen: Vec<Id<InterfaceNode>> = Vec::new();
    for base in &service.bases {
        collect_inherited(ast, base.id, &mut seen, &mut out, 0)?;
    }
    out.extend(
        service
            .all_operations()
            .into_iter()
            .map(|operation| (service.name.clone(), service.module, operation)),
    );
    Ok(out)
}

/// zerodds-lint: recursion-depth 64 (one level per base interface;
/// `depth` refuses a chain at `MAX_INHERITANCE_DEPTH`, and `seen`
/// admits each interface once)
fn collect_inherited(
    ast: &Ast,
    interface: Id<InterfaceNode>,
    seen: &mut Vec<Id<InterfaceNode>>,
    out: &mut Vec<DeclaredOperation>,
    depth: usize,
) -> Result<(), InterfaceError> {
    if seen.contains(&interface) {
        return Ok(());
    }
    let this = service(ast, interface)?;
    if depth >= MAX_INHERITANCE_DEPTH {
        return Err(InterfaceError::TooDeep {
            name: this.name.clone(),
            limit: MAX_INHERITANCE_DEPTH,
        });
    }
    seen.push(interface);
    for base in &this.bases {
        collect_inherited(ast, base.id, seen, out, depth + 1)?;
    }
    for operation in this.all_operations() {
        out.push((this.name.clone(), this.module, operation));
    }
    Ok(())
}

/// `@Autoid` structure over `members`; a hash collision is an error
/// (§7.5.1.2.1.2).
fn autoid_struct(
    name: String,
    choice: bool,
    mutable: bool,
    empty: bool,
    members: Vec<(String, EnhancedMemberType)>,
) -> Result<EnhancedStruct, BindingError> {
    let mut built: Vec<EnhancedMember> = Vec::with_capacity(members.len());
    for (member_name, type_) in members {
        let id = member_id(&member_name);
        if let Some(clash) = built.iter().find(|member| member.member_id == id) {
            return Err(BindingError::Codec(format!(
                "members `{}` and `{member_name}` of `{name}` hash to the same member id {id:#010x}",
                clash.name
            )));
        }
        built.push(EnhancedMember {
            name: member_name,
            type_,
            member_id: id,
        });
    }
    Ok(EnhancedStruct {
        name,
        choice,
        autoid: true,
        empty,
        mutable,
        members: built,
    })
}

/// The name of the return value's member in an `Out` structure.
/// §7.5.1.1.5 rule 3b: `return_`, or `return_N` where an `out` parameter
/// has that name.
fn return_member_name(taken: &[String]) -> String {
    if !taken.iter().any(|name| name == RETURN_MEMBER) {
        return RETURN_MEMBER.to_string();
    }
    for n in 1..=i32::MAX {
        let candidate = format!("{RETURN_MEMBER}{n}");
        if !taken.contains(&candidate) {
            return candidate;
        }
    }
    format!("{RETURN_MEMBER} ")
}

/// `remoteEx`, or `remoteEx_N` where an operation has that name
/// (§7.5.1.2.5 rule 5).
fn remote_ex_member_name(operations: &[EnhancedOperation]) -> String {
    let taken = |candidate: &str| {
        operations
            .iter()
            .any(|operation| operation.operation.name == candidate)
    };
    if !taken(REMOTE_EX_MEMBER) {
        return REMOTE_EX_MEMBER.to_string();
    }
    for n in 1..=i32::MAX {
        let candidate = format!("{REMOTE_EX_MEMBER}_{n}");
        if !taken(&candidate) {
            return candidate;
        }
    }
    format!("{REMOTE_EX_MEMBER} ")
}

/// The Enhanced Service Mapping of `service` (DDS-RPC 1.0 §7.5.1.2).
///
/// # Errors
/// [`BindingError::Interface`] as [`service`]; [`BindingError::Codec`]
/// for two operations of one name, or two members of an `@Autoid`
/// structure that hash to one member id.
pub fn enhanced_service(
    ast: &Ast,
    service: &Service,
    empty: EmptyOperationStyle,
) -> Result<EnhancedService, BindingError> {
    let empty_annotation = empty == EmptyOperationStyle::EmptyAnnotation;
    let mut operations: Vec<EnhancedOperation> = Vec::new();
    for (interface, module, operation) in operations_with_interface_flattened(ast, service)? {
        if operations
            .iter()
            .any(|seen| seen.operation.name == operation.name)
        {
            return Err(BindingError::Codec(format!(
                "operation `{}` is declared twice in `{}` and its bases",
                operation.name, service.name
            )));
        }
        // §7.5.1.2.2: the rules of §7.5.1.1.4 rule 3.
        let mut in_members: Vec<(String, EnhancedMemberType)> = operation
            .inputs()
            .map(|param| (param.name.clone(), EnhancedMemberType::Idl(param.type_)))
            .collect();
        let in_empty = in_members.is_empty();
        if in_empty && !empty_annotation {
            in_members.push((DUMMY_MEMBER.to_string(), EnhancedMemberType::Dummy));
        }
        // §7.5.1.2.3 rule 1: the rules of §7.5.1.1.5 rule 3.
        let mut out_members: Vec<(String, EnhancedMemberType)> = operation
            .outputs()
            .map(|param| (param.name.clone(), EnhancedMemberType::Idl(param.type_)))
            .collect();
        if let Some(returns) = operation.returns {
            let taken: Vec<String> = out_members.iter().map(|(name, _)| name.clone()).collect();
            out_members.push((return_member_name(&taken), EnhancedMemberType::Idl(returns)));
        }
        let out_empty = out_members.is_empty();
        if out_empty && !empty_annotation {
            out_members.push((DUMMY_MEMBER.to_string(), EnhancedMemberType::Dummy));
        }
        let in_struct = autoid_struct(
            in_struct_name(&interface, &operation.name),
            false,
            operation.mutable,
            in_empty && empty_annotation,
            in_members,
        )?;
        let out_struct = autoid_struct(
            out_struct_name(&interface, &operation.name),
            false,
            operation.mutable,
            out_empty && empty_annotation,
            out_members,
        )?;
        // §7.5.1.2.3 rules 2 to 7.
        let mut result_members = vec![(
            RESULT_MEMBER.to_string(),
            EnhancedMemberType::Struct(out_struct.name.clone()),
        )];
        for raised in &operation.raises {
            result_members.push((
                exception_member_name(&raised.name),
                EnhancedMemberType::Exception(raised.clone()),
            ));
        }
        let result_struct = autoid_struct(
            result_union_name(&interface, &operation.name),
            true,
            operation.mutable,
            false,
            result_members,
        )?;
        operations.push(EnhancedOperation {
            interface,
            module,
            operation,
            in_struct,
            out_struct,
            result_struct,
        });
    }

    // §7.5.1.2.4: a member per operation, typed `…_In`.
    let request = autoid_struct(
        request_type_name(&service.name),
        true,
        false,
        false,
        operations
            .iter()
            .map(|operation| {
                (
                    operation.operation.name.clone(),
                    EnhancedMemberType::Struct(operation.in_struct.name.clone()),
                )
            })
            .collect(),
    )?;
    // §7.5.1.2.5: a member per operation, typed `…_Result`, and `remoteEx`.
    let mut reply_members: Vec<(String, EnhancedMemberType)> = operations
        .iter()
        .map(|operation| {
            (
                operation.operation.name.clone(),
                EnhancedMemberType::Struct(operation.result_struct.name.clone()),
            )
        })
        .collect();
    reply_members.push((
        remote_ex_member_name(&operations),
        EnhancedMemberType::RemoteException,
    ));
    let reply = autoid_struct(
        reply_type_name(&service.name),
        true,
        false,
        false,
        reply_members,
    )?;
    Ok(EnhancedService {
        operations,
        request,
        reply,
    })
}

/// The `<interface_name>` of §7.4.1 for one interface node.
fn qualified_name_of(ast: &Ast, node: &InterfaceNode) -> Result<String, InterfaceError> {
    let chain = module_chain(ast, node.module)?;
    let segments: Vec<&str> = chain.iter().map(|(_, name)| name.as_str()).collect();
    Ok(qualified_interface_name(&segments, &node.name))
}

/// The `name = "…"` of `@service`, wherever the frontend put it.
///
/// The IDL frontend folds `@service` into the CORBA facet — IDL 4.2
/// §8.3.4 declares it there — rather than leaving it among the applied
/// annotations, so a reader that looked only at `facets.annotations`
/// finds nothing and every service silently takes the `"Service"`
/// default. `crates/rpc/src/annotations.rs::lower_rpc_annotations` reads
/// both places for the same reason.
///
/// Only the explicit `name = "…"` form is read: `@service("DDS")` names
/// a *platform*, and treating that as a service name would rename every
/// service that declares one.
fn service_annotation_name(ast: &Ast, facets: &zerodds_ast::Facets) -> Option<String> {
    if let Some(corba) = facets.corba.as_ref()
        && corba.is_service
        && let Some(name) = named_string_argument(ast, &corba.service_arguments, "name")
    {
        return Some(name);
    }
    let applied = applied_annotation(facets, "service")?;
    named_string_argument(ast, &applied.arguments, "name")
}

/// The applied annotation with this simple name, if the facets carry
/// one.
///
/// A leading underscore comes off: IDL 4.2 §7.2.3.2 makes it "a purely
/// lexical convention that ONLY turns off keyword checking".
fn applied_annotation<'a>(
    facets: &'a zerodds_ast::Facets,
    annotation: &str,
) -> Option<&'a zerodds_ast::AppliedAnnotation> {
    facets.annotations.iter().find(|candidate| {
        let tail = candidate
            .name
            .rsplit("::")
            .next()
            .unwrap_or(&candidate.name);
        tail.strip_prefix('_').unwrap_or(tail) == annotation
    })
}

/// The string of the argument written `key = "…"`.
fn named_string_argument(
    ast: &Ast,
    arguments: &[zerodds_ast::AppliedArgument],
    key: &str,
) -> Option<String> {
    let argument = arguments
        .iter()
        .find(|argument| argument.name.as_deref() == Some(key))?;
    literal_string(ast, argument.value)
}

/// The string a constant expression evaluates to, if it is one.
fn literal_string(ast: &Ast, id: Id<zerodds_ast::ConstExprNode>) -> Option<String> {
    match &ast.expr(id).ok()?.expr {
        zerodds_ast::ConstExpr::Literal(zerodds_ast::Literal::String(text)) => Some(text.clone()),
        _ => None,
    }
}

/// The string argument of one applied annotation, by the annotation's
/// simple name and the argument's key.
///
/// The generator's reader of the three annotations §7.4 gives it —
/// `@service(name = "…")`, `@DDSRequestTopic(name = "…")` and
/// `@DDSReplyTopic(name = "…")`. `crates/rpc/src/annotations.rs` reads
/// the same three for the run time and reads more besides; the two are
/// held against each other by
/// `crates/rpc/tests/language_binding_agreement.rs`, because two readers
/// of one annotation that disagree would put a client and a service on
/// two topics.
///
/// A leading underscore comes off the annotation's name: IDL 4.2
/// §7.2.3.2 makes it "a purely lexical convention that ONLY turns off
/// keyword checking".
fn annotation_string(
    ast: &Ast,
    facets: &zerodds_ast::Facets,
    annotation: &str,
    key: &str,
) -> Option<String> {
    let applied = applied_annotation(facets, annotation)?;
    // `@ann("x")` addresses the declaration's `value` member (IDL 4.2
    // §8.2); `@ann(key = "x")` names it. Both are read, the positional
    // one first, which is the order `crates/rpc/src/annotations.rs`
    // takes.
    let argument = applied
        .arguments
        .iter()
        .find(|argument| argument.name.is_none())
        .or_else(|| {
            applied
                .arguments
                .iter()
                .find(|argument| argument.name.as_deref() == Some(key))
        })?;
    literal_string(ast, argument.value)
}

/// The string argument `key` of one applied annotation, by name only: an
/// annotation with several string attributes (`@Qos`) has no positional
/// one to take for any of them.
fn annotation_named_string(
    ast: &Ast,
    facets: &zerodds_ast::Facets,
    annotation: &str,
    key: &str,
) -> Option<String> {
    let applied = applied_annotation(facets, annotation)?;
    let argument = applied
        .arguments
        .iter()
        .find(|argument| argument.name.as_deref() == Some(key))?;
    literal_string(ast, argument.value)
}

fn one_operation(
    ast: &Ast,
    interface: &str,
    op: &OperationNode,
) -> Result<Operation, InterfaceError> {
    let at = format!("{interface}::{}", op.name);
    // Production (87) removed `oneway` and `context`; §7.5.1.1.7 rule 5
    // gives every operation a Return-union case, so a `oneway` operation
    // would have a case for a reply it never sends.
    if op.oneway || applied_annotation(&op.facets, "oneway").is_some() {
        return Err(InterfaceError::OutsideServiceGrammar {
            construct: "a `oneway` operation",
            at,
        });
    }
    if !op.contexts.is_empty() {
        return Err(InterfaceError::OutsideServiceGrammar {
            construct: "a `context` clause",
            at,
        });
    }
    let returns = match &op.return_type {
        None => None,
        Some(reference) => {
            Some(
                reference
                    .resolved()
                    .ok_or_else(|| InterfaceError::Unresolved {
                        construct: "return type",
                        at: at.clone(),
                    })?,
            )
        }
    };

    let mut params = Vec::with_capacity(op.params.len());
    for param in &op.params {
        let node = ast.param(*param).map_err(|_| InterfaceError::NoNode {
            kind: "parameter",
            at: at.clone(),
        })?;
        let type_ = node
            .type_
            .resolved()
            .ok_or_else(|| InterfaceError::Unresolved {
                construct: "parameter type",
                at: format!("{at}({})", node.name),
            })?;
        if params
            .iter()
            .any(|other: &Parameter| other.name == node.name)
        {
            return Err(InterfaceError::DuplicateParameter {
                at,
                name: node.name.clone(),
            });
        }
        params.push(Parameter {
            name: node.name.clone(),
            direction: node.direction,
            type_,
        });
    }

    Ok(Operation {
        node_id: Some(op.node_id),
        name: op.name.clone(),
        returns,
        params,
        raises: raises(ast, &at, "raises", &op.raises)?,
        origin: Origin::Declared,
        mutable: applied_annotation(&op.facets, "Mutable").is_some(),
    })
}

fn one_attribute(
    ast: &Ast,
    interface: &str,
    attr: &AttributeNode,
) -> Result<Attribute, InterfaceError> {
    let at = format!("{interface}::{}", attr.name);
    let type_ = attr
        .type_
        .resolved()
        .ok_or_else(|| InterfaceError::Unresolved {
            construct: "attribute type",
            at: at.clone(),
        })?;
    Ok(Attribute {
        node_id: attr.node_id,
        name: attr.name.clone(),
        type_,
        readonly: attr.readonly,
        get_raises: raises(ast, &at, "getraises", &attr.get_raises)?,
        set_raises: raises(ast, &at, "setraises", &attr.set_raises)?,
    })
}

fn raises(
    ast: &Ast,
    at: &str,
    clause: &'static str,
    list: &[zerodds_ast_core::Ref<ExceptionNode>],
) -> Result<Vec<Raise>, InterfaceError> {
    let mut out = Vec::with_capacity(list.len());
    for reference in list {
        let id = reference.resolved().ok_or(InterfaceError::Unresolved {
            construct: clause,
            at: at.to_string(),
        })?;
        let node = ast.exception(id).map_err(|_| InterfaceError::NoNode {
            kind: "exception",
            at: at.to_string(),
        })?;
        let chain = module_chain(ast, node.module)?;
        let mut scoped: Vec<&str> = chain.iter().map(|(_, name)| name.as_str()).collect();
        scoped.push(&node.name);
        out.push(Raise {
            id,
            scoped_name: scoped.join("::"),
            name: node.name.clone(),
        });
    }
    Ok(out)
}
