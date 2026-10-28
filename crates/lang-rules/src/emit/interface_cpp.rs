// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! An IDL `interface`, as the C++ language binding DDS-RPC 1.0 §7.11.1
//! specifies.
//!
//! `adapter::interface` reads the interface; this writes the C++. The
//! split is the one every column in this crate keeps — one reading of
//! the IDL clause, one spelling per language — and here it earns itself
//! twice over, because §7.11.2.3 makes the Java function-call binding
//! *the same entities as C++* and `emit::interface_java` therefore walks
//! the same [`Service`] values.
//!
//! ## What comes out
//!
//! Per interface `I`, in the namespace its module names:
//!
//! - the topic types of §7.5.1.1.4-7 — `I_<op>_In`, `I_<op>_Out`,
//!   `I_Call`, `I_Return`, `I_Request`, `I_Reply` — whose names come
//!   from `adapter::interface` and whose shape is
//!   `crates/rpc/src/codegen.rs`'s;
//! - `ISupport`, so §7.11.1.1.2 rule 3's `SupportType` typedef names
//!   something that exists;
//! - `I` and `IAsync`, the two abstract classes of rules 1-7;
//! - `IRequester` and `IReplier`, the typed aliases of §7.11.1.1.1 over
//!   the generic `dds::rpc::Requester` / `dds::rpc::Replier` templates;
//! - `IClient` and `IService`, the function-call entities of §7.11.1.5.
//!
//! ## Reserved words
//!
//! §7.11.1.1.2 rule 8 prefixes an identifier that is a C++ reserved word
//! with `cxx_`. This binding does that for every identifier it introduces:
//! the interface, its operations, its parameters. The type declarations
//! come from `adapter::cpp`, which gives a struct, an enumeration, an
//! exception or a module named by a reserved word the same prefix
//! (`adapter::cpp::cxx_name`); this binding refers to them by that spelling.
//!
//! ## What this does not do
//!
//! It does not implement `dds::rpc`. §7.11.1.4 lists some twenty class
//! templates — `Requester`, `Replier`, `future`, `LoanedSamples` and the
//! rest — which are the C++ PSM's runtime and not a per-service
//! artefact. What is generated here uses them: `IClient`'s operations
//! call through a `Requester`, `IService::dispatch` answers through a
//! `Replier`, and the topic types have the codec the two ask for
//! (`mod codec`, `mod calls`).

mod calls;
mod codec;
mod enhanced;
mod enhanced_codec;

use crate::adapter::cpp::{
    CppMapping, NATIVE_NAMESPACE, cpp_mapping, cpp_primitive, cpp_value_type, module_names,
    nest_by_module, nest_by_module_placed,
};
use crate::adapter::cpp_psm::psm_type;
use crate::adapter::interface::{
    self, BindingError, Operation, Origin, Parameter, RETURN_MEMBER, Service, ServiceMapping,
};
use zerodds_ast::{Ast, ModuleNode, ParamDirection, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Path};
use zerodds_lang_cst::cpp;

/// The namespace the DDS-RPC C++ runtime lives in. DDS-RPC 1.0
/// §7.11.1.4.1: "The `dds`, and `dds::rpc` namespaces define the classes
/// and functions for the request-reply style language bindings."
const RPC: &str = "dds::rpc";

/// How deep an alias chain is followed before the model is taken to hold
/// a cycle. The same bound `adapter::cpp` uses for a type spelling.
const MAX_ALIAS_DEPTH: usize = 64;

/// The declarations an interface produces, each with the module whose
/// namespace it stands in and whether it is a topic type.
///
/// Under `--cpp-mapping psm` the topic types (the `In`, `Out`, `Result`,
/// `Call`, `Return`, `Request` and `Reply` structures and the case labels)
/// are the native types the codec reads and writes: they stand in
/// `zd_native` (DDS-PSM-Cxx 1.0 §8.1). The interfaces, the clients and the
/// services are the application's and stand in the namespaces of the IDL,
/// which see the topic types through a `using` declaration each.
pub(super) struct Sink {
    items: Vec<(Id<ModuleNode>, bool, cpp::Decl)>,
    /// Topic types the application's namespace does not see through a
    /// `using` declaration, because it declares a type of its own by that name.
    unseen: Vec<(Id<ModuleNode>, String)>,
}

impl Sink {
    fn new() -> Self {
        Self {
            items: Vec::new(),
            unseen: Vec::new(),
        }
    }

    /// A topic type the application's namespace declares its own type for
    /// (the `Out` structure under the PSM mapping, whose members the
    /// application sees as Table 7.1 spells them): no `using` for it.
    pub(super) fn push_topic_unseen(&mut self, item: (Id<ModuleNode>, cpp::Decl)) {
        if let Some(topic) = topic_name(&item.1) {
            self.unseen.push((item.0, topic));
        }
        self.items.push((item.0, true, item.1));
    }

    /// A declaration of the application, or one at file scope.
    pub(super) fn push(&mut self, item: (Id<ModuleNode>, cpp::Decl)) {
        self.items.push((item.0, false, item.1));
    }

    /// A topic type or a case label.
    pub(super) fn push_topic(&mut self, item: (Id<ModuleNode>, cpp::Decl)) {
        self.items.push((item.0, true, item.1));
    }

    /// The declarations, nested into the namespaces of their modules.
    fn nest(self, ast: &Ast) -> Result<Vec<cpp::Decl>, BindingError> {
        if !is_psm() {
            let plain = self
                .items
                .into_iter()
                .map(|(module, _, decl)| (module, decl))
                .collect();
            return nest_by_module(ast, plain).map_err(BindingError::from);
        }
        let mut placed = Vec::with_capacity(self.items.len());
        let unseen = self.unseen;
        for (module, native, decl) in self.items {
            let using = if native {
                topic_name(&decl)
                    .filter(|topic| {
                        !unseen
                            .iter()
                            .any(|(at, name)| *at == module && name == topic)
                    })
                    .map(|topic| {
                        let scope = module_names(ast, module)?;
                        let mut path = format!("::{NATIVE_NAMESPACE}");
                        for segment in scope {
                            path.push_str("::");
                            path.push_str(&segment);
                        }
                        Ok::<_, crate::adapter::cst::MaterialiseError>(cpp::Decl::Verbatim(
                            format!("using {path}::{topic};"),
                        ))
                    })
                    .transpose()?
            } else {
                None
            };
            placed.push((module, native, decl));
            if let Some(using) = using {
                placed.push((module, false, using));
            }
        }
        nest_by_module_placed(ast, placed).map_err(BindingError::from)
    }
}

/// The name a topic type or a case label is declared under.
fn topic_name(decl: &cpp::Decl) -> Option<String> {
    if let cpp::Decl::Class(class) = decl {
        return Some(class.name.as_str().to_string());
    }
    if let cpp::Decl::Variable(variable) = decl {
        return Some(variable.name.as_str().to_string());
    }
    None
}

/// The C++ declarations every interface of `ast` produces.
///
/// The list is module-nested, exactly as [`crate::adapter::cpp::cpp_items`]
/// returns its own, so a caller appends the two and renders one file.
///
/// # Errors
/// [`BindingError`] — the interface did not reach the seam, or a type in
/// it has no C++ spelling. Never a dropped operation.
pub fn cpp_interface_items(ast: &Ast) -> Result<Vec<cpp::Decl>, BindingError> {
    cpp_interface_items_with(ast, ServiceMapping::Basic)
}

/// The types the operations of the remote interfaces of `ast` carry (the
/// parameters, the return values and the members of the exceptions they
/// raise), each once: the types whose codecs a binding of the interfaces
/// calls, in any language.
///
/// # Errors
/// [`BindingError`] — the interface did not reach the seam.
pub fn remote_member_types(ast: &Ast) -> Result<Vec<Id<TypeNode>>, BindingError> {
    let remote: Vec<Service> = interface::services(ast)?
        .into_iter()
        .filter(Service::is_remote)
        .collect();
    codec::member_types(ast, &remote)
}

/// [`cpp_interface_items`] for the given service mapping.
///
/// [`ServiceMapping::Enhanced`] writes the topic types of DDS-RPC 1.0
/// §7.5.1.2 — the `@Choice @Autoid` `Request` and `Reply` structures and
/// the `In`, `Out` and `Result` structures — their PL_CDR2 codec, the
/// interface classes and the function-call classes over the structures
/// (`${I}Client`, `${I}Service`); the Basic mapping's `Call` union is not
/// written.
///
/// # Errors
/// As [`cpp_interface_items`].
pub fn cpp_interface_items_with(
    ast: &Ast,
    mapping: ServiceMapping,
) -> Result<Vec<cpp::Decl>, BindingError> {
    let mut out = Sink::new();
    let services = interface::services(ast)?;
    let root = ast.root_module();
    if services.iter().any(Service::is_remote) {
        // The classes below name `dds::rpc` and the topic types' codecs
        // name `zerodds::wire`; a header that includes neither is not one a
        // consumer can include on its own.
        out.push((
            root,
            cpp::Decl::Verbatim("#include <dds/rpc.hpp>".to_string()),
        ));
        let remote: Vec<Service> = services
            .iter()
            .filter(|service| service.is_remote())
            .cloned()
            .collect();
        for decl in codec::member_codecs(ast, &remote)? {
            out.push((root, decl));
        }
        // The Basic mapping is classic CDR unless the params say XCDR2
        // (§7.5.1.1): a nested `@appendable` or `@mutable` member is
        // written differently there, so the codecs exist under both.
        if !mapping.is_enhanced() {
            let classic = codec::member_codecs_xcdr1(ast, &remote)?;
            if !classic.is_empty() {
                out.push((root, codec::in_xcdr1_namespace(classic)));
            }
        }
    }
    // §7.5.1.1.5 rule 4a names the constant after the exception alone, so
    // two interfaces of one module that raise it share the one constant.
    let mut declared_hashes: Vec<(Id<ModuleNode>, String)> = Vec::new();
    let mut declared_exceptions = Vec::new();
    for service in services {
        if mapping.is_enhanced() {
            enhanced::declare(
                ast,
                &Service { mapping, ..service },
                &mut out,
                &mut declared_exceptions,
            )?;
        } else {
            declare(ast, &service, &mut out, &mut declared_hashes)?;
        }
    }
    out.nest(ast)
}

/// The C++ declarations one interface produces.
///
/// # Errors
/// As [`cpp_interface_items`].
pub fn cpp_service_items(ast: &Ast, service: &Service) -> Result<Vec<cpp::Decl>, BindingError> {
    let mut out = Sink::new();
    declare(ast, service, &mut out, &mut Vec::new())?;
    out.nest(ast)
}

fn declare(
    ast: &Ast,
    service: &Service,
    out: &mut Sink,
    declared_hashes: &mut Vec<(Id<ModuleNode>, String)>,
) -> Result<(), BindingError> {
    let module = service.module;
    let name = cxx_escape(&service.name);
    let operations = service.all_operations();

    // The topic types first: everything below names them, and a C++
    // class whose member names a class the compiler has not seen does
    // not compile.
    for operation in &operations {
        out.push_topic((
            module,
            cpp::Decl::Class(struct_of(
                &interface::in_struct_name(&name, &cxx_escape(&operation.name)),
                request_fields(ast, service, operation)?,
                Doc::line("The `in` and `inout` parameters of one call. DDS-RPC 1.0 §7.5.1.1.4."),
            )),
        ));
        let out_struct = (
            module,
            cpp::Decl::Class(struct_of(
                &interface::out_struct_name(&name, &cxx_escape(&operation.name)),
                reply_fields(ast, service, operation)?,
                Doc::line(
                    "The return value and the `out` and `inout` parameters of one call. DDS-RPC 1.0 §7.5.1.1.5.",
                ),
            )),
        );
        if is_psm() && operation.has_outputs() {
            // The `future` of an asynchronous call holds this structure
            // (rule 7.i), and the application sees its members as Table 7.1
            // spells them: the topic structure is native, in `zd_native`, and
            // the application's own `Out` follows it in the IDL's namespace.
            out.push_topic_unseen(out_struct);
            out.push((
                module,
                cpp::Decl::Class(struct_of(
                    &interface::out_struct_name(&name, &cxx_escape(&operation.name)),
                    reply_fields_for_application(ast, service, operation)?,
                    Doc::line("The return value and the `out` and `inout` parameters of one call, as the application sees them. DDS-RPC 1.0 §7.5.1.1.5 and §7.11.1.1.2 rule 7.i."),
                )),
            ));
        } else {
            out.push_topic(out_struct);
        }
    }

    // §7.5.1.1.6 rule 4 and §7.5.1.1.5 rule 4a: the case labels are
    // constants beside the unions, one per operation and one per
    // exception, so a reader of the header can see which value selects
    // which case without recomputing an MD5.
    for operation in &operations {
        let operation_name = cxx_escape(&operation.name);
        out.push_topic((
            module,
            hash_constant(
                &interface::operation_hash_constant(&name, &operation_name),
                interface::operation_hash(&operation.name),
                "The case label of one operation. DDS-RPC 1.0 §7.5.1.1.6 rules 4 and 5a.",
            ),
        ));
    }
    for operation in &operations {
        for raised in &operation.raises {
            let key = (module, raised.scoped_name.clone());
            if declared_hashes.contains(&key) {
                continue;
            }
            declared_hashes.push(key);
            out.push_topic((
                module,
                hash_constant(
                    &interface::exception_hash_constant(&cxx_escape(&raised.name)),
                    interface::exception_hash(&raised.scoped_name),
                    "The case label of one raised exception. DDS-RPC 1.0 §7.5.1.1.5 rule 4.",
                ),
            ));
        }
    }

    // §7.5.1.1.5: one `Result` union per operation, over the `Out`
    // structure and the exceptions the operation raises.
    for operation in &operations {
        out.push_topic((
            module,
            cpp::Decl::Class(result_union_of(service, &name, operation)),
        ));
    }

    out.push_topic((
        module,
        cpp::Decl::Class(union_of(
            &interface::call_union_name(&name),
            &operations,
            &name,
            false,
            Doc::line("The request union over every operation. DDS-RPC 1.0 §7.5.1.1.6."),
        )),
    ));
    out.push_topic((
        module,
        cpp::Decl::Class(union_of(
            &interface::return_union_name(&name),
            &operations,
            &name,
            true,
            Doc::line("The reply union. DDS-RPC 1.0 §7.5.1.1.7."),
        )),
    ));

    let request = interface::request_type_name(&name);
    let reply = interface::reply_type_name(&name);
    out.push_topic((
        module,
        cpp::Decl::Class(struct_of(
            &request,
            vec![
                field(qualified(RPC, "RequestHeader"), "header"),
                field(
                    cpp::Type::named(Path::single(&interface::call_union_name(&name))),
                    interface::DATA_MEMBER,
                ),
            ],
            Doc::line("The request topic type. DDS-RPC 1.0 §7.5.1.1.6."),
        )),
    ));
    out.push_topic((
        module,
        cpp::Decl::Class(struct_of(
            &reply,
            vec![
                field(qualified(RPC, "ReplyHeader"), "header"),
                field(
                    cpp::Type::named(Path::single(&interface::return_union_name(&name))),
                    interface::DATA_MEMBER,
                ),
            ],
            Doc::line("The reply topic type. DDS-RPC 1.0 §7.5.1.1.7."),
        )),
    ));
    if service.is_remote() {
        // The octets of the two topic types: what `dds::rpc::Requester` and
        // `dds::rpc::Replier` ask `dds_type_traits` for.
        let root = ast.root_module();
        let synthesized = codec::synthesized_codecs(ast, service, &name, &operations)?;
        for decl in synthesized.iter().cloned() {
            out.push((root, decl));
        }
        out.push((root, codec::in_xcdr1_namespace(synthesized)));
        out.push((root, codec::traits(ast, service, &name)?));
    }
    out.push((module, cpp::Decl::Class(support_class(service, &name))));

    // `I` names `IAsync` in a typedef and `IAsync` names `I` in one, so
    // one of the two has to be announced before the other is defined.
    out.push((
        module,
        cpp::Decl::Forward {
            is_struct: false,
            name: Ident::new(&format!("{name}Async")),
        },
    ));
    out.push((
        module,
        cpp::Decl::Class(sync_interface(ast, service, &name, &operations)?),
    ));
    out.push((
        module,
        cpp::Decl::Class(async_interface(ast, service, &name, &operations)?),
    ));

    if service.is_remote() {
        out.push((module, requester_alias(&name, &request, &reply)));
        out.push((module, replier_alias(&name, &request, &reply)));
        out.push((
            module,
            cpp::Decl::Class(calls::client_class(
                ast,
                service,
                &name,
                &operations,
                calls::Shape::Basic,
            )?),
        ));
        out.push((
            module,
            cpp::Decl::Class(calls::service_class(
                ast,
                service,
                &name,
                &operations,
                calls::Shape::Basic,
            )?),
        ));
    }
    Ok(())
}

// -- the topic types ---------------------------------------------------

fn request_fields(
    ast: &Ast,
    service: &Service,
    operation: &Operation,
) -> Result<Vec<cpp::ClassMember>, BindingError> {
    let mut out = Vec::new();
    for param in operation.inputs() {
        out.push(field(
            cpp_value_type(ast, param.type_, &at(service, operation, &param.name))?,
            &cxx_escape(&param.name),
        ));
    }
    // §7.5.1.1.4 rule 3c: an operation with no `in`/`inout` parameters
    // carries a single `dummy` of type `dds::rpc::UnusedMember`, not an
    // empty structure — XCDR gives an empty structure no octets, and a
    // reader cannot tell a sample that arrived from one that did not.
    if out.is_empty() {
        out.push(field(
            qualified(RPC, "UnusedMember"),
            interface::DUMMY_MEMBER,
        ));
    }
    Ok(out)
}

/// The members of the `Out` structure as the application sees them under
/// the PSM mapping: [`reply_fields`] with Table 7.1's spellings.
fn reply_fields_for_application(
    ast: &Ast,
    service: &Service,
    operation: &Operation,
) -> Result<Vec<cpp::ClassMember>, BindingError> {
    let mut out = Vec::new();
    for param in operation.outputs() {
        out.push(field(
            user_type(ast, param.type_, &at(service, operation, &param.name))?,
            &cxx_escape(&param.name),
        ));
    }
    if let Some(returns) = operation.returns {
        let taken: Vec<String> = operation
            .outputs()
            .map(|param| cxx_escape(&param.name))
            .collect();
        out.push(field(
            user_type(ast, returns, &at(service, operation, "return"))?,
            &return_member_name(&taken),
        ));
    }
    Ok(out)
}

/// The native `Out` structure of `operation`, spelled so that it is not
/// taken for the application's own of that name.
pub(super) fn native_out_type(
    ast: &Ast,
    module: Id<ModuleNode>,
    name: &str,
    operation: &Operation,
) -> Result<cpp::Type, BindingError> {
    let out = interface::out_struct_name(name, &cxx_escape(&operation.name));
    if !is_psm() {
        return Ok(cpp::Type::named(Path::single(&out)));
    }
    let mut path = format!("::{NATIVE_NAMESPACE}");
    for segment in module_names(ast, module).map_err(BindingError::from)? {
        path.push_str("::");
        path.push_str(&segment);
    }
    path.push_str("::");
    path.push_str(&out);
    Ok(cpp::Type::named(Path::single(&path)))
}

fn reply_fields(
    ast: &Ast,
    service: &Service,
    operation: &Operation,
) -> Result<Vec<cpp::ClassMember>, BindingError> {
    // §7.5.1.1.5 rule 3a: the `out` and `inout` parameters first, in
    // declaration order and under their formal names; rule 3b: the
    // return value *last*.
    let mut out = Vec::new();
    for param in operation.outputs() {
        out.push(field(
            cpp_value_type(ast, param.type_, &at(service, operation, &param.name))?,
            &cxx_escape(&param.name),
        ));
    }
    if let Some(returns) = operation.returns {
        let taken: Vec<String> = operation
            .outputs()
            .map(|param| cxx_escape(&param.name))
            .collect();
        out.push(field(
            cpp_value_type(ast, returns, &at(service, operation, "return"))?,
            &return_member_name(&taken),
        ));
    }
    // Rule 3c: nothing at all becomes the `dummy` member.
    if out.is_empty() {
        out.push(field(
            qualified(RPC, "UnusedMember"),
            interface::DUMMY_MEMBER,
        ));
    }
    Ok(out)
}

/// The name §7.5.1.1.5 rule 3b gives the return value, with the
/// collision rule: "If an operation has an out argument named `return_`
/// and the operation has a regular return value, the regular return
/// value in the Out structure shall be represented as `return_N` member
/// of the appropriate type where N is the first integer in the range 1
/// to 2^31 that avoids the collision."
fn return_member_name(taken: &[String]) -> String {
    if !taken.iter().any(|name| name == RETURN_MEMBER) {
        return RETURN_MEMBER.to_string();
    }
    for n in 1..=i32::MAX {
        let candidate = format!("{RETURN_MEMBER}{n}");
        if !taken.iter().any(|name| name == &candidate) {
            return candidate;
        }
    }
    // Unreachable below 2^31 parameters; a name no IDL identifier can
    // take is still better than two members sharing one.
    format!("{RETURN_MEMBER} ")
}

/// The call union, as a class with a discriminator and one field per
/// case.
///
/// A class and not a C++ `union`, for the reason
/// `crate::adapter::cpp`'s module documentation gives about IDL unions:
/// two cases may carry the same type, and a member of a `union` with a
/// non-trivial constructor — which `std::string` and `std::vector` both
/// have — has to be placed and destroyed by hand.
fn union_of(
    name: &str,
    operations: &[Operation],
    service: &str,
    reply: bool,
    doc: Doc,
) -> cpp::Class {
    let mut members = vec![
        discriminator_field(),
        // §7.5.1.1.6 rule 3: "The Call union shall have a default case
        // label containing a member named `unknownOp` of type
        // `dds::rpc::UnknownOperation`." Without it a client built
        // against a newer interface selects a case the service does not
        // have, and the sample decodes as whichever case happens to be
        // first.
        case_field(
            qualified(RPC, "UnknownOperation"),
            interface::UNKNOWN_OP_MEMBER,
            Doc::line("Live on the `default` case. DDS-RPC 1.0 §7.5.1.1.6 rule 3."),
        ),
    ];
    for operation in operations {
        let operation_name = cxx_escape(&operation.name);
        let case = if reply {
            interface::result_union_name(service, &operation_name)
        } else {
            interface::in_struct_name(service, &operation_name)
        };
        members.push(case_field(
            cpp::Type::named(Path::single(&case)),
            &operation_name,
            Doc::line(&format!(
                "Live when `_d` is `{}`.",
                interface::operation_hash_constant(service, &operation_name)
            )),
        ));
    }
    cpp::Class {
        doc,
        is_struct: true,
        name: Ident::new(name),
        bases: Vec::new(),
        sections: vec![cpp::Section {
            access: cpp::Access::Public,
            members,
        }],
    }
}

/// The `_d` member every synthesized union carries.
///
/// The discriminator of §7.5.1.1.6 rule 2 is an IDL `long`, so it takes
/// the spelling `adapter::cpp` gives one and not the bare `int32_t` the
/// CST would also accept: two spellings of one width in one header is a
/// diff waiting to be read as a difference.
fn discriminator_field() -> cpp::ClassMember {
    cpp::ClassMember::Field(cpp::Field {
        doc: Doc::line("Which case this sample carries. DDS-RPC 1.0 §7.5.1.1.6 rule 2."),
        type_: cpp_primitive(PrimitiveKind::Int32),
        name: Ident::new("_d"),
        dims: Vec::new(),
        default: None,
        is_static: false,
        is_constexpr: false,
        bits: None,
    })
}

/// One case member of a synthesized union.
fn case_field(type_: cpp::Type, name: &str, doc: Doc) -> cpp::ClassMember {
    cpp::ClassMember::Field(cpp::Field {
        doc,
        type_,
        name: Ident::new(name),
        dims: Vec::new(),
        default: None,
        is_static: false,
        is_constexpr: false,
        bits: None,
    })
}

/// One `${I}_${op}_Hash` or `${exception}_Ex_Hash` constant.
///
/// `inline constexpr` and not a macro, for the reason
/// `crate::adapter::cpp` gives about IDL constants: an inline variable
/// is one object across the program and warns nowhere it is unused.
fn hash_constant(name: &str, value: i32, doc: &str) -> cpp::Decl {
    cpp::Decl::Variable(cpp::Variable {
        doc: Doc::line(doc),
        type_: cpp_primitive(PrimitiveKind::Int32),
        name: Ident::new(name),
        value: Some(cpp::Expr::Literal(zerodds_lang_cst::common::Literal::Int(
            value.to_string(),
        ))),
        is_static: false,
        is_constexpr: true,
        is_inline: true,
    })
}

/// `${I}_${op}_Result` — §7.5.1.1.5.
///
/// The `dds::RETCODE_OK` case carries the `Out` structure under
/// `result`; every exception the operation raises carries its own,
/// under the lower-cased name with `_ex` appended.
fn result_union_of(service: &Service, name: &str, operation: &Operation) -> cpp::Class {
    let operation_name = cxx_escape(&operation.name);
    let mut members = vec![
        discriminator_field(),
        case_field(
            cpp::Type::named(Path::single(&interface::out_struct_name(
                name,
                &operation_name,
            ))),
            interface::RESULT_MEMBER,
            Doc::line(&format!(
                "Live when `_d` is `dds::RETCODE_OK` ({}). DDS-RPC 1.0 §7.5.1.1.5 rule 3a.",
                interface::RETCODE_OK
            )),
        ),
    ];
    for raised in &operation.raises {
        members.push(case_field(
            cpp::Type::named(Path::single(&cxx_escape(&raised.name))),
            // Rule 5b names the member after the IDL exception; `<name>_ex`
            // is not a reserved word, so rule 8 does not prefix it.
            &interface::exception_member_name(&raised.name),
            Doc::line(&format!(
                "Live when `_d` is `{}`. DDS-RPC 1.0 §7.5.1.1.5 rule 5.",
                interface::exception_hash_constant(&cxx_escape(&raised.name))
            )),
        ));
    }
    let _ = service;
    cpp::Class {
        doc: Doc::line("The outcome of one operation. DDS-RPC 1.0 §7.5.1.1.5."),
        is_struct: true,
        name: Ident::new(&interface::result_union_name(name, &operation_name)),
        bases: Vec::new(),
        sections: vec![cpp::Section {
            access: cpp::Access::Public,
            members,
        }],
    }
}

/// `ISupport` — what §7.11.1.1.2 rule 3's `SupportType` typedef names.
///
/// The rule requires the typedef; the DDS-Cxx-PSM type support behind it
/// is generated by the type-support pass, which is why this carries the
/// two things a service's support type has to answer and no more: which
/// topic types the service uses, and what its two topics are called
/// (§7.4.1.1).
fn support_class(service: &Service, name: &str) -> cpp::Class {
    // §7.4.1's two names, which are not the type names: the topic is
    // `<interface_name>_<service_name>_Request` with the interface name
    // fully qualified, and the type is `${interfaceName}_Request`
    // (§7.5.1.1.6). `SupportType` names both so a caller never has to
    // build either.
    let style = interface::BindingStyle::FunctionCall;
    let mut members = vec![
        alias("RequestType", &interface::request_type_name(name)),
        alias("ReplyType", &interface::reply_type_name(name)),
        topic_name_method(
            "request_topic_name",
            &service.request_topic_name(style),
            "The request topic's name. DDS-RPC 1.0 §7.4.1, §7.4.2.",
        ),
        topic_name_method(
            "reply_topic_name",
            &service.reply_topic_name(style),
            "The reply topic's name. DDS-RPC 1.0 §7.4.1, §7.4.2.",
        ),
        topic_name_method(
            "service_name",
            &service.service_name,
            "The `<service_name>` segment of §7.4.1, `\"Service\"` unless \
             `@service(name = \"…\")` supplied one.",
        ),
    ];
    members.push(calls::defaults_method(service));
    // §7.4.3: one alias per parent interface, announced through
    // `PID_TOPIC_ALIASES` at discovery.
    for (at, alias_name) in service.topic_aliases(style, false).into_iter().enumerate() {
        members.push(topic_name_method(
            &format!("request_topic_alias_{at}"),
            &alias_name,
            "A request-topic alias from a parent interface. DDS-RPC 1.0 §7.4.3.",
        ));
    }
    for (at, alias_name) in service.topic_aliases(style, true).into_iter().enumerate() {
        members.push(topic_name_method(
            &format!("reply_topic_alias_{at}"),
            &alias_name,
            "A reply-topic alias from a parent interface. DDS-RPC 1.0 §7.4.3.",
        ));
    }
    cpp::Class {
        doc: Doc::line("The service's type support: its topic types and their topic names."),
        is_struct: true,
        name: Ident::new(&format!("{name}Support")),
        bases: Vec::new(),
        sections: vec![cpp::Section {
            access: cpp::Access::Public,
            members,
        }],
    }
}

fn topic_name_method(method: &str, topic: &str, doc: &str) -> cpp::ClassMember {
    cpp::ClassMember::Method(cpp::Method {
        doc: Doc::line(doc),
        returns: cpp::Type::Pointer(Box::new(cpp::Type::Const(Box::new(cpp::Type::Primitive(
            cpp::Primitive::Char,
        ))))),
        name: Ident::new(method),
        params: Vec::new(),
        is_const: false,
        is_static: true,
        is_virtual: false,
        is_pure: false,
        is_constexpr: true,
        is_noexcept: true,
        is_override: false,
        body: Some(cpp::Block {
            statements: vec![cpp::Statement::Return(Some(cpp::Expr::Literal(
                zerodds_lang_cst::common::Literal::Str(topic.to_string()),
            )))],
        }),
    })
}

// -- the two abstract classes ------------------------------------------

/// `class I` — §7.11.1.1.2 rules 1-6.
fn sync_interface(
    ast: &Ast,
    service: &Service,
    name: &str,
    operations: &[Operation],
) -> Result<cpp::Class, BindingError> {
    let mut members = vec![
        alias("SupportType", &format!("{name}Support")),
        alias("RequestType", &interface::request_type_name(name)),
        alias("ReplyType", &interface::reply_type_name(name)),
        // Rule 4.
        alias("AsyncInterfaceType", &format!("{name}Async")),
        // Rule 2.
        cpp::ClassMember::Destructor(cpp::Destructor {
            doc: Doc::line("Rule 2: a public virtual destructor."),
            name: Ident::new(name),
            is_virtual: true,
            body: cpp::FunctionBody::Defaulted,
        }),
    ];
    let _ = operations;
    for operation in &service.operations {
        members.push(cpp::ClassMember::Method(sync_method(
            ast, service, operation,
        )?));
    }
    // Rule 6.f sends the attributes to DDS-Cxx-PSM §7.4.6, which names
    // the functions after the attribute rather than `get_`/`set_`. The
    // `get_`/`set_` names of DDS-RPC §7.5.1.1.3 are the *implied IDL*,
    // and the implied IDL is what the topic types above are built from —
    // the two are one clause apart and name two different things.
    for attribute in &service.attributes {
        let at = format!("{}::{}", service.name, attribute.name);
        let type_ = user_type(ast, attribute.type_, &at)?;
        let spelled = cxx_escape(&attribute.name);
        if is_native(ast, attribute.type_) {
            // `NT attribute();` and `void attribute(NT attrib);`
            members.push(cpp::ClassMember::Method(attribute_method(
                &spelled,
                type_.clone(),
                Vec::new(),
                false,
            )));
            if !attribute.readonly {
                members.push(cpp::ClassMember::Method(attribute_method(
                    &spelled,
                    cpp::Type::Void,
                    vec![cpp::Param {
                        type_,
                        name: Some(Ident::new(&spelled)),
                        default: None,
                    }],
                    false,
                )));
            }
        } else {
            // `CT& attribute();`, `const CT& attribute() const;` and
            // `void attribute(const CT& attrib);`
            members.push(cpp::ClassMember::Method(attribute_method(
                &spelled,
                cpp::Type::Ref(Box::new(type_.clone())),
                Vec::new(),
                false,
            )));
            members.push(cpp::ClassMember::Method(attribute_method(
                &spelled,
                cpp::Type::Ref(Box::new(cpp::Type::Const(Box::new(type_.clone())))),
                Vec::new(),
                true,
            )));
            if !attribute.readonly {
                members.push(cpp::ClassMember::Method(attribute_method(
                    &spelled,
                    cpp::Type::Void,
                    vec![cpp::Param {
                        type_: cpp::Type::Ref(Box::new(cpp::Type::Const(Box::new(type_)))),
                        name: Some(Ident::new(&spelled)),
                        default: None,
                    }],
                    false,
                )));
            }
        }
    }
    Ok(cpp::Class {
        doc: Doc::line(
            "The service, as the abstract class DDS-RPC 1.0 §7.11.1.1.2 rule 1 requires.",
        ),
        is_struct: false,
        name: Ident::new(name),
        bases: bases(ast, service, "")?,
        sections: vec![cpp::Section {
            access: cpp::Access::Public,
            members,
        }],
    })
}

/// One synchronous operation — rule 6.
fn sync_method(
    ast: &Ast,
    service: &Service,
    operation: &Operation,
) -> Result<cpp::Method, BindingError> {
    let mut params = Vec::new();
    let mut returns = cpp::Type::Void;

    // Rule 6.h: an operation returning a constructed type takes it as a
    // leading `T&` named `cxx_return` and returns `void`. Rule 6.g: a
    // primitive or an enumeration comes back by value.
    if let Some(target) = operation.returns {
        let type_ = user_type(ast, target, &at(service, operation, "return"))?;
        if is_native(ast, target) {
            returns = type_;
        } else {
            params.push(cpp::Param {
                type_: cpp::Type::Ref(Box::new(type_)),
                name: Some(Ident::new("cxx_return")),
                default: None,
            });
        }
    }

    // Rule 6.b: the number and the order of the arguments are the IDL's.
    // Rule 6.e: DDS-Cxx-PSM §7.4.5 decides the form each takes.
    for param in &operation.params {
        params.push(cpp::Param {
            type_: passed(ast, param, &at(service, operation, &param.name))?,
            name: Some(Ident::new(&cxx_escape(&param.name))),
            default: None,
        });
    }

    Ok(cpp::Method {
        doc: doc_for(operation),
        returns,
        name: Ident::new(&cxx_escape(&operation.name)),
        params,
        is_const: false,
        is_static: false,
        is_virtual: true,
        is_pure: true,
        is_constexpr: false,
        // Rule 6.c: no exception specification, whatever the IDL raises.
        is_noexcept: false,
        is_override: false,
        body: None,
    })
}

/// `class IAsync` — §7.11.1.1.2 rules 1-5 and 7.
fn async_interface(
    ast: &Ast,
    service: &Service,
    name: &str,
    operations: &[Operation],
) -> Result<cpp::Class, BindingError> {
    let mut members = vec![
        alias("SupportType", &format!("{name}Support")),
        alias("RequestType", &interface::request_type_name(name)),
        alias("ReplyType", &interface::reply_type_name(name)),
        // Rule 5.
        alias("InterfaceType", name),
        cpp::ClassMember::Destructor(cpp::Destructor {
            doc: Doc::line("Rule 2: a public virtual destructor."),
            name: Ident::new(&format!("{name}Async")),
            is_virtual: true,
            body: cpp::FunctionBody::Defaulted,
        }),
    ];
    let _ = operations;
    for operation in &service.operations {
        members.push(cpp::ClassMember::Method(async_method(
            ast, service, name, operation,
        )?));
    }
    // Rule 7.f: "Attribute getter functions shall take no parameters and
    // shall return `dds::rpc::future` of the same type as the
    // attribute." Rule 7.g: a setter takes its parameter per
    // DDS-Cxx-PSM §7.4.6 and returns `dds::rpc::future<void>`. Rule 7.a
    // gives both the `_async` suffix, over the §7.4.6 name.
    for attribute in &service.attributes {
        let at = format!("{}::{}", service.name, attribute.name);
        // The future holds the value as the topic type does; the argument of
        // the setter is what the application passes.
        let payload = user_type(ast, attribute.type_, &at)?;
        let type_ = user_type(ast, attribute.type_, &at)?;
        let spelled = cxx_escape(&attribute.name);
        members.push(cpp::ClassMember::Method(async_attribute_method(
            &spelled,
            future_of(payload),
            Vec::new(),
        )));
        if !attribute.readonly {
            let passed = if is_native(ast, attribute.type_) {
                cpp::Type::Const(Box::new(type_))
            } else {
                cpp::Type::Ref(Box::new(cpp::Type::Const(Box::new(type_))))
            };
            members.push(cpp::ClassMember::Method(async_attribute_method(
                &spelled,
                future_of(cpp::Type::Void),
                vec![cpp::Param {
                    type_: passed,
                    name: Some(Ident::new(&spelled)),
                    default: None,
                }],
            )));
        }
    }
    let bases = bases(ast, service, "Async")?;
    Ok(cpp::Class {
        doc: Doc::line(
            "The service's asynchronous half, as DDS-RPC 1.0 §7.11.1.1.2 rule 1 requires.",
        ),
        is_struct: false,
        name: Ident::new(&format!("{name}Async")),
        bases,
        sections: vec![cpp::Section {
            access: cpp::Access::Public,
            members,
        }],
    })
}

/// What the `future` of an asynchronous operation holds.
///
/// Rule 7.i: an operation with `out` or `inout` parameters comes back as a
/// `future` of the reply struct, because a `future` has one value and the
/// call has several. Rule 7.h: one with an ordinary return comes back as
/// a `future` of that. Otherwise `future<void>`.
fn async_payload(
    ast: &Ast,
    service: &Service,
    name: &str,
    operation: &Operation,
) -> Result<cpp::Type, BindingError> {
    Ok(if operation.has_outputs() {
        cpp::Type::named(Path::single(&interface::out_struct_name(
            name,
            &cxx_escape(&operation.name),
        )))
    } else if let Some(target) = operation.returns {
        // The application's type of the value (Table 7.1 under the PSM
        // mapping), as the synchronous call returns it.
        user_type(ast, target, &at(service, operation, "return"))?
    } else {
        cpp::Type::Void
    })
}

/// One asynchronous operation — rule 7.
fn async_method(
    ast: &Ast,
    service: &Service,
    name: &str,
    operation: &Operation,
) -> Result<cpp::Method, BindingError> {
    let payload = async_payload(ast, service, name, operation)?;

    // Rule 7.b: `In` and `InOut` only, no `Out`. Rule 7.d/7.e: every
    // argument is `const`.
    let mut params = Vec::new();
    for param in operation.inputs() {
        let type_ = user_type(ast, param.type_, &at(service, operation, &param.name))?;
        params.push(cpp::Param {
            type_: if is_native(ast, param.type_) {
                cpp::Type::Const(Box::new(type_))
            } else {
                cpp::Type::Ref(Box::new(cpp::Type::Const(Box::new(type_))))
            },
            name: Some(Ident::new(&cxx_escape(&param.name))),
            default: None,
        });
    }

    Ok(cpp::Method {
        doc: doc_for(operation),
        returns: future_of(payload),
        // Rule 7.a.
        name: Ident::new(&format!("{}_async", cxx_escape(&operation.name))),
        params,
        is_const: false,
        is_static: false,
        is_virtual: true,
        is_pure: true,
        is_constexpr: false,
        is_noexcept: false,
        is_override: false,
        body: None,
    })
}

// -- the endpoints -----------------------------------------------------

/// `using IRequester = dds::rpc::core::Requester<I_Request, I_Reply>;`
///
/// §7.11.1.1.1: the binding "shall use … the structs synthesized from an
/// IDL interface", and the `Requester` template "provide[s] a uniform
/// interface to send, receive, allocate, initialize, and destroy data".
/// A per-service alias and not a per-service subclass: there is nothing
/// for a subclass to add that the two type arguments do not already say,
/// and an alias is the same type, so a `Requester<I_Request, I_Reply>`
/// obtained anywhere else is the same object.
fn requester_alias(name: &str, request: &str, reply: &str) -> cpp::Decl {
    endpoint_alias(&format!("{name}Requester"), "Requester", request, reply)
}

/// `using IReplier = dds::rpc::core::Replier<I_Request, I_Reply>;` — §7.11.1.1.1.
fn replier_alias(name: &str, request: &str, reply: &str) -> cpp::Decl {
    endpoint_alias(&format!("{name}Replier"), "Replier", request, reply)
}

fn endpoint_alias(name: &str, template: &str, request: &str, reply: &str) -> cpp::Decl {
    cpp::Decl::Alias(cpp::Alias {
        doc: Doc::line(&format!(
            "The service's `{template}`, over its two topic types. DDS-RPC 1.0 §7.11.1.1.1."
        )),
        name: Ident::new(name),
        type_: cpp::Type::template(
            Path::new(&["dds", "rpc", "core", template]),
            vec![
                cpp::Type::named(Path::single(request)),
                cpp::Type::named(Path::single(reply)),
            ],
        ),
    })
}

// -- the shapes the rules are written in -------------------------------

/// Whether the types of the application are the PSM's (`--cpp-mapping psm`).
pub(super) fn is_psm() -> bool {
    cpp_mapping() == CppMapping::Psm
}

/// The type a parameter, a return value or an attribute has where the
/// application sees it.
///
/// Under the native mapping that is the type the topic structures hold.
/// Under the PSM mapping (DDS-PSM-Cxx 1.0 §8.1) it is the PSM's spelling of
/// Table 7.1 — `std::wstring` for `wstring`, `std::map` for `map`,
/// `dds::core::array` for an array — and the generated bodies convert
/// between it and the topic structures with `zerodds::psm::Conv`.
pub(super) fn user_type(ast: &Ast, id: Id<TypeNode>, at: &str) -> Result<cpp::Type, BindingError> {
    if is_psm() {
        return Ok(cpp::Type::named(Path::single(&psm_type(ast, id, at)?)));
    }
    Ok(cpp_value_type(ast, id, at)?)
}

/// `value`, a value of the topic structures' type `id`, as the application
/// sees it; or `value` as the topic structures hold it. `value` itself under
/// the native mapping.
pub(super) fn converted(
    ast: &Ast,
    id: Id<TypeNode>,
    at: &str,
    value: cpp::Expr,
    to_native: bool,
) -> Result<cpp::Expr, BindingError> {
    if !is_psm() {
        return Ok(value);
    }
    let conv = format!("Conv<{}>", psm_type(ast, id, at)?);
    let function = if to_native {
        "to_native"
    } else {
        "from_native"
    };
    Ok(cpp::Expr::Call {
        callee: Box::new(cpp::Expr::Path(Path::new(&[
            "zerodds",
            "psm",
            conv.as_str(),
            function,
        ]))),
        args: vec![value],
    })
}

/// DDS-Cxx-PSM §7.4.5's table, for one parameter.
///
/// A native type passes `IN` by value and `OUT`/`INOUT` as `T&`; a
/// constructed one passes `IN` as `const T&` and the other two as `T&`.
fn passed(ast: &Ast, param: &Parameter, at: &str) -> Result<cpp::Type, BindingError> {
    let type_ = user_type(ast, param.type_, at)?;
    Ok(match param.direction {
        ParamDirection::In if is_native(ast, param.type_) => type_,
        ParamDirection::In => cpp::Type::Ref(Box::new(cpp::Type::Const(Box::new(type_)))),
        ParamDirection::Out | ParamDirection::InOut => cpp::Type::Ref(Box::new(type_)),
    })
}

/// Whether the C++ PSM's table calls this a *native* type.
///
/// §7.4.5 splits its table by "native" against "type", and
/// §7.11.1.1.2 rule 6.g names "primitive types and enumeration types"
/// together as what an operation returns by value. A bitmask and a
/// bitset are integer holders in `adapter::cpp` and belong on the same
/// side; a string, a sequence, an array, a map, a struct, a union and a
/// valuetype do not.
fn is_native(ast: &Ast, id: Id<TypeNode>) -> bool {
    native_at(ast, id, 0)
}

/// zerodds-lint: recursion-depth 64 (alias chains; bounded by `MAX_ALIAS_DEPTH`)
fn native_at(ast: &Ast, id: Id<TypeNode>, depth: usize) -> bool {
    if depth > MAX_ALIAS_DEPTH {
        return false;
    }
    let Ok(node) = ast.type_(id) else {
        return false;
    };
    match &node.kind {
        TypeKind::Primitive(_) | TypeKind::Enumeration { .. } | TypeKind::Bitmask { .. } => true,
        TypeKind::Alias { aliased } => aliased
            .resolved()
            .is_some_and(|target| native_at(ast, target, depth + 1)),
        _ => false,
    }
}

/// The base classes an interface's abstract class carries.
///
/// IDL inheritance, carried into the mapping: §7.11.1.1.2 rule 6 gives
/// `${interface}` "pure virtual functions for all the operations and
/// attributes defined in the IDL", and the ones a base defines are
/// defined in the IDL of the base. §7.5.1.1.8 keeps the *topics* per
/// hierarchy level, which is why the topic types above carry only this
/// interface's own operations.
fn bases(ast: &Ast, service: &Service, suffix: &str) -> Result<Vec<cpp::Base>, BindingError> {
    service
        .bases
        .iter()
        .map(|base| {
            // The base may be declared in another module.
            let module = interface::service(ast, base.id)?.module;
            Ok(cpp::Base {
                access: cpp::Access::Public,
                virtual_: false,
                path: enhanced::path_in_idl(
                    ast,
                    service.module,
                    module,
                    &format!("{}{suffix}", cxx_escape(&base.name)),
                )?,
            })
        })
        .collect()
}

/// One accessor of DDS-Cxx-PSM §7.4.6's table.
fn attribute_method(
    name: &str,
    returns: cpp::Type,
    params: Vec<cpp::Param>,
    is_const: bool,
) -> cpp::Method {
    cpp::Method {
        doc: Doc::line("An attribute accessor. DDS-Cxx-PSM 1.0 §7.4.6."),
        returns,
        name: Ident::new(name),
        params,
        is_const,
        is_static: false,
        is_virtual: true,
        is_pure: true,
        is_constexpr: false,
        is_noexcept: false,
        is_override: false,
        body: None,
    }
}

/// One asynchronous accessor — rules 7.a, 7.f and 7.g.
fn async_attribute_method(name: &str, returns: cpp::Type, params: Vec<cpp::Param>) -> cpp::Method {
    cpp::Method {
        doc: Doc::line("An attribute accessor. DDS-RPC 1.0 §7.11.1.1.2 rules 7.f and 7.g."),
        returns,
        name: Ident::new(&format!("{name}_async")),
        params,
        is_const: false,
        is_static: false,
        is_virtual: true,
        is_pure: true,
        is_constexpr: false,
        is_noexcept: false,
        is_override: false,
        body: None,
    }
}

/// `dds::rpc::future<T>` — §7.11.1.4.13.
fn future_of(payload: cpp::Type) -> cpp::Type {
    cpp::Type::template(Path::new(&["dds", "rpc", "future"]), vec![payload])
}

fn struct_of(name: &str, members: Vec<cpp::ClassMember>, doc: Doc) -> cpp::Class {
    cpp::Class {
        doc,
        is_struct: true,
        name: Ident::new(name),
        bases: Vec::new(),
        sections: vec![cpp::Section {
            access: cpp::Access::Public,
            members,
        }],
    }
}

fn field(type_: cpp::Type, name: &str) -> cpp::ClassMember {
    cpp::ClassMember::Field(cpp::Field {
        doc: Doc::none(),
        type_,
        name: Ident::new(name),
        dims: Vec::new(),
        default: None,
        is_static: false,
        is_constexpr: false,
        bits: None,
    })
}

fn alias(name: &str, target: &str) -> cpp::ClassMember {
    cpp::ClassMember::Alias(cpp::Alias {
        doc: Doc::none(),
        name: Ident::new(name),
        type_: cpp::Type::named(Path::single(target)),
    })
}

fn qualified(namespace: &str, name: &str) -> cpp::Type {
    let mut segments: Vec<&str> = namespace.split("::").collect();
    segments.push(name);
    cpp::Type::named(Path::new(&segments))
}

/// The `raises` clause, as documentation. See
/// [`Operation::raises_line`].
fn doc_for(operation: &Operation) -> Doc {
    operation
        .raises_line()
        .map_or_else(Doc::none, |line| Doc::line(&line))
}

fn at(service: &Service, operation: &Operation, what: &str) -> String {
    format!("{}::{}({what})", service.name, operation.name)
}

/// DDS-RPC 1.0 §7.11.1.1.2 rule 8.
///
/// "Any identifier (e.g., parameter, operation name, exception,
/// interface, module) in an IDL file that is a reserved word in C++
/// shall be prefixed by `cxx_`."
#[must_use]
pub fn cxx_escape(name: &str) -> String {
    // One definition of the prefix rule, shared with the declarations
    // `adapter::cpp` writes, so a name and every reference to it agree.
    crate::adapter::cpp::cxx_name(name)
}
