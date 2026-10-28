// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! An IDL `interface`, as a complete DDS-RPC 1.0 Rust binding: the
//! request and reply topic types of §7.5.1.1, a typed client and
//! service trait, and a dispatcher — all runnable, not only declared.
//!
//! `adapter::interface` reads the interface; `emit::interface_cpp` and
//! `emit::interface_java` write declarations a peer's own runtime gives
//! a body to. This writes both halves: the topic types get a real
//! XCDR2 codec, hand-assembled with the same per-field writers
//! `crate::encode::rust_codec` uses for an ordinary `struct`, and the
//! client and the dispatcher are built over `zerodds_rpc::requester`
//! and `zerodds_rpc::replier` — the crate this generated file has to
//! depend on, named in the preamble every generated interface carries.
//!
//! ## Why the topic types get a codec here and nowhere else
//!
//! C++, Java and TypeScript declare `Calculator_add_In` and stop: the
//! octets are a job for a type-support pass none of the three has
//! written (`emit::interface_cpp`'s own module doc says so). Rust's
//! generated client and service actually run — `crates/rpc/tests/`
//! sends a request and reads a reply — and a `Requester<TIn, TOut>`
//! needs `TIn: DdsType`. So this crosses the boundary `zerodds-rpc →
//! zerodds-dcps → zerodds-lang-rules` warns about, deliberately: not by
//! making this *crate* depend on `zerodds-rpc` — it still does not —
//! but by writing Rust source that does, exactly as
//! `crates/zerodds-build/src/dds_type.rs` writes an `impl
//! zerodds_dcps::DdsType` for a type this crate declared.
//!
//! ## The wire layout
//!
//! DDS-RPC 1.0 §7.5.1.1's Basic Service Mapping: one request topic type
//! and one reply topic type per interface, each a `header` plus a
//! `data` union over every operation. `crates/rpc/src/codegen.rs` is
//! the data model this shape comes from; this crate cannot depend on
//! it (`zerodds-rpc` depends on `zerodds-dcps` depends on this crate),
//! so the shape is read off `adapter::interface::Service` directly, the
//! way `emit::interface_cpp` already does for C++.
//!
//! A union becomes a Rust `enum`, one variant per case plus a default
//! `UnknownOp(u8)` — §7.5.1.1.6 rule 3 and §7.5.1.1.7 rule 3 — and its
//! `encode`/`decode` write the `long` discriminator
//! [`interface::operation_hash`]/[`interface::exception_hash`] compute,
//! then delegate to the case payload's own codec. Every payload type —
//! an `_In`, an `_Out`, a `_Result`, a raised exception — gets exactly
//! that: an inherent `encode(&self, w: &mut Writer)` and `decode(r: &mut
//! Reader)`, member by member, through `crate::encode::write_value_to`
//! and `crate::encode::read_for` — the same two functions
//! `crate::encode::rust_codec` calls for a declared `struct`.
//!
//! Every synthesized type is `@final`: no `DHEADER`, matching
//! `crates/rpc/src/common_types.rs`'s own choice for `RequestHeader`
//! and `ReplyHeader`, and matching the classic-CDR shape DDS-RPC 1.0
//! predates XTypes 1.3's extensibility kinds with. The encoding is
//! fixed to XCDR2 little-endian regardless of the file's own
//! `--encoding`/`--endian` — [`rpc_layout_key`] — because
//! `zerodds_rpc::requester::Requester::send_request_blocking` calls
//! `TIn::encode`, the `DdsType` bridge's un-suffixed method, which is
//! always that one cell.
//!
//! ## What this does not do
//!
//! It reads `@service(name = "…")`, `@DDSRequestTopic` and
//! `@DDSReplyTopic` (`annotated_topics_expr`) and hands the names to
//! `zerodds_rpc::requester::Requester::with_options` as
//! `AnnotatedTopics`; `crates/rpc/src/topic_naming.rs` composes the topic
//! names of §7.4.1 and §7.4.2 from them.

mod enhanced;
pub mod type_information;

use core::fmt;
use std::collections::BTreeSet;

use zerodds_ast::{
    Ast, ExceptionNode, ModuleNode, ParamDirection, PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::Id;
use zerodds_ast_layout::key::{Encoding, Endianness, LayoutKey};
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path, Visibility};
use zerodds_lang_cst::rust;

use crate::adapter::cst::{MaterialiseError, data_derives, rust_type_of};
use crate::adapter::interface::{
    self, BindingError, InterfaceError, Operation, RETURN_MEMBER, Raise, Service,
};
use crate::encode::{EncodeError, MemberWrite, Position, read_for, write_of, write_value_to};

/// Why an interface did not become a Rust binding.
#[derive(Debug)]
pub enum RustInterfaceError {
    /// The interface itself did not reach the seam.
    Interface(InterfaceError),
    /// A referenced type has no Rust spelling.
    Materialise(MaterialiseError),
    /// A field's codec could not be built.
    Codec(EncodeError),
}

impl From<InterfaceError> for RustInterfaceError {
    fn from(error: InterfaceError) -> Self {
        Self::Interface(error)
    }
}
impl From<BindingError> for RustInterfaceError {
    fn from(error: BindingError) -> Self {
        match error {
            BindingError::Interface(error) => Self::Interface(error),
            BindingError::Materialise(error) => Self::Materialise(error),
            // Only the C++ column builds codecs through the seam; the
            // reason is kept.
            BindingError::Codec(reason) => Self::Materialise(MaterialiseError::UnknownType {
                type_name: format!("a codec: {reason}"),
            }),
        }
    }
}
impl From<MaterialiseError> for RustInterfaceError {
    fn from(error: MaterialiseError) -> Self {
        Self::Materialise(error)
    }
}
impl From<EncodeError> for RustInterfaceError {
    fn from(error: EncodeError) -> Self {
        Self::Codec(error)
    }
}

impl fmt::Display for RustInterfaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Interface(error) => write!(f, "{error}"),
            Self::Materialise(error) => write!(f, "{error:?}"),
            Self::Codec(error) => write!(f, "{error}"),
        }
    }
}

/// The `LayoutKey` every synthesized topic type's codec is built under.
/// See the module doc.
#[must_use]
pub fn rpc_layout_key() -> LayoutKey {
    LayoutKey::data(Encoding::PlainCdr2, Endianness::Little)
}

/// The `LayoutKey` of the XCDR1 (classic CDR) codec of every synthesized
/// topic type, the second pair `CodecVariant::XCDR1` writes. DDS-RPC 1.0
/// §7.5.1.1: the Basic Service Mapping is for DDS 1.3.
#[must_use]
pub fn rpc_layout_key_xcdr1() -> LayoutKey {
    LayoutKey::data(Encoding::PlainCdr1, Endianness::Little)
}

/// Which of a topic type's two codec pairs a function writes or calls.
///
/// The XCDR2 pair has the bare names `encode`/`decode`; the XCDR1 pair
/// carries `crate::encode::SECONDARY_SUFFIX` (`encode_xcdr1`,
/// `decode_xcdr1`), as `crate::encode::rust_codec_pair` names the second
/// pair of a declared type, so a member that is a declared type calls the
/// pair of its own version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CodecVariant {
    Xcdr2,
    Xcdr1,
}

impl CodecVariant {
    fn suffix(self) -> &'static str {
        match self {
            Self::Xcdr2 => "",
            Self::Xcdr1 => crate::encode::SECONDARY_SUFFIX,
        }
    }

    fn key(self) -> LayoutKey {
        match self {
            Self::Xcdr2 => rpc_layout_key(),
            Self::Xcdr1 => rpc_layout_key_xcdr1(),
        }
    }

    fn encode_name(self) -> String {
        format!("encode{}", self.suffix())
    }

    fn decode_name(self) -> String {
        format!("decode{}", self.suffix())
    }
}

/// The Rust declarations and runnable binding every interface of `ast`
/// produces, each paired with the module it belongs in so a caller can
/// fold it into the same tree `crate::adapter::cst::nest_by_module`
/// builds for everything else.
///
/// # Errors
/// [`RustInterfaceError`] — the interface did not reach the seam, a
/// referenced type has no Rust spelling, or a field's codec could not
/// be built. Never a dropped operation, parameter, attribute or raised
/// exception.
pub fn rust_interface_pairs(
    ast: &Ast,
) -> Result<Vec<(Id<ModuleNode>, rust::Item)>, RustInterfaceError> {
    rust_interface_pairs_with(ast, interface::ServiceMapping::Basic)
}

/// [`rust_interface_pairs`] for the given service mapping.
///
/// [`interface::ServiceMapping::Enhanced`] writes the topic types of
/// DDS-RPC 1.0 §7.5.1.2 — the `@Choice @Autoid` `Request` and `Reply`
/// and the `In`, `Out` and `Result` structures, with their PL_CDR2 codec
/// — and a client and a dispatcher that use
/// `zerodds_rpc::endpoint_options::RpcMapping::Enhanced`.
///
/// # Errors
/// As [`rust_interface_pairs`].
pub fn rust_interface_pairs_with(
    ast: &Ast,
    mapping: interface::ServiceMapping,
) -> Result<Vec<(Id<ModuleNode>, rust::Item)>, RustInterfaceError> {
    let mut out = Vec::new();
    let capable = xcdr1_capable_types(ast);
    for service in interface::services(ast)? {
        if mapping.is_enhanced() {
            enhanced::declare(ast, &Service { mapping, ..service }, &mut out)?;
        } else {
            declare(ast, &service, &capable, &mut out)?;
        }
    }
    Ok(out)
}

/// The declared types that have an XCDR1 codec pair
/// (`crate::encode::rust_codec_pair` under [`rpc_layout_key_xcdr1`]).
/// `crate::generate` emits those pairs for a model with interfaces, and the
/// topic types' XCDR1 codec calls them.
#[must_use]
pub fn xcdr1_capable_types(ast: &Ast) -> BTreeSet<Id<TypeNode>> {
    ast.types()
        .filter(|(id, node)| {
            !node.name.is_empty()
                && crate::encode::has_codec(&node.kind)
                && crate::encode::rust_codec_pair(ast, *id, rpc_layout_key_xcdr1()).is_ok()
        })
        .map(|(id, _)| id)
        .collect()
}

/// The most distinct declared types one reachability walk visits. The
/// visited set bounds the recursion; past this many the walk answers
/// "not closed", which withholds the XCDR1 form and makes the Basic default
/// fail with an error rather than send wrong octets.
const MAX_REACHABLE_TYPES: usize = 4096;

/// Whether every declared type reachable from `target` has an XCDR1 pair.
///
/// zerodds-lint: recursion-depth 4096 (each type is entered once; the
/// visited set is capped at `MAX_REACHABLE_TYPES`)
pub(crate) fn type_closed(
    ast: &Ast,
    target: Id<TypeNode>,
    capable: &BTreeSet<Id<TypeNode>>,
    seen: &mut BTreeSet<Id<TypeNode>>,
) -> bool {
    if !seen.insert(target) {
        return true;
    }
    if seen.len() > MAX_REACHABLE_TYPES {
        return false;
    }
    let Ok(node) = ast.type_(target) else {
        return false;
    };
    if crate::encode::has_codec(&node.kind) && !capable.contains(&target) {
        return false;
    }
    let reaches = |reference: &zerodds_ast_core::Ref<TypeNode>,
                   seen: &mut BTreeSet<Id<TypeNode>>| {
        reference
            .resolved()
            .is_some_and(|id| type_closed(ast, id, capable, seen))
    };
    let member_closed = |member: Id<zerodds_ast::MemberNode>, seen: &mut BTreeSet<Id<TypeNode>>| {
        ast.member(member)
            .is_ok_and(|member| reaches(&member.type_, seen))
    };
    match &node.kind {
        TypeKind::Structure { base, members } => {
            base.as_ref().is_none_or(|base| reaches(base, seen))
                && members.iter().all(|m| member_closed(*m, seen))
        }
        TypeKind::Union {
            discriminator,
            cases,
            ..
        } => {
            reaches(discriminator, seen)
                && cases.iter().all(|case| {
                    ast.case(*case)
                        .is_ok_and(|case| member_closed(case.member, seen))
                })
        }
        TypeKind::Alias { aliased } => reaches(aliased, seen),
        TypeKind::Sequence { element, .. } | TypeKind::Array { element, .. } => {
            reaches(element, seen)
        }
        TypeKind::Map { key, value, .. } => reaches(key, seen) && reaches(value, seen),
        TypeKind::ValueBox { boxed } => reaches(boxed, seen),
        TypeKind::Valuetype { bases, state, .. } => {
            bases.iter().all(|b| reaches(b, seen)) && state.iter().all(|m| member_closed(*m, seen))
        }
        _ => true,
    }
}

fn fields_closed(ast: &Ast, fields: &[FieldSpec], capable: &BTreeSet<Id<TypeNode>>) -> bool {
    fields.iter().all(|field| match field {
        FieldSpec::Octet(_) => true,
        FieldSpec::Real(_, target) => type_closed(ast, *target, capable, &mut BTreeSet::new()),
    })
}

fn declare(
    ast: &Ast,
    service: &Service,
    capable: &BTreeSet<Id<TypeNode>>,
    out: &mut Vec<(Id<ModuleNode>, rust::Item)>,
) -> Result<(), RustInterfaceError> {
    let module = service.module;
    let ops = service.all_operations();

    // The exceptions this interface's operations raise, each declared
    // (as a plain struct, by `crate::adapter::cst`'s own walk of
    // `ast.exceptions()`) but with no codec of its own — nothing before
    // this reads an exception outside a C++/Java header. One codec per
    // exception, deduplicated, in the exception's own module: two
    // operations that raise the same exception must not each get their
    // own `impl DivByZero`, which is `error[E0592]`.
    //
    // Each codec has a second pair for XCDR1 (classic CDR, the Basic
    // mapping's encoding, DDS-RPC 1.0 §7.5.1.1), written only where every
    // declared type it reaches has one: `xcdr1_ok` records which of them
    // does, and a topic type without an XCDR1 pair leaves the `DdsType`
    // XCDR1 methods at the trait's refusing defaults.
    let mut exception_ok: Vec<(Id<ExceptionNode>, bool)> = Vec::new();
    for raise in exceptions_of(&ops) {
        let exc_module = exception_module(ast, raise.id)?;
        out.push((exc_module, exception_codec_item(ast, raise.id)?));
        let ok = exception_fields_closed(ast, raise.id, capable)?;
        if ok {
            out.push((
                exc_module,
                exception_codec_item_with(ast, raise.id, CodecVariant::Xcdr1)?,
            ));
        }
        exception_ok.push((raise.id, ok));
    }

    let mut in_ok: Vec<bool> = Vec::new();
    let mut result_ok: Vec<bool> = Vec::new();
    for op in &ops {
        out.push((module, rust::Item::Struct(in_struct(ast, service, op)?)));
        let in_name = interface::in_struct_name(&service.name, &op.name);
        out.push((
            module,
            struct_codec_item(ast, module, &in_name, &in_fields(op))?,
        ));
        let ok = fields_closed(ast, &in_fields(op), capable);
        if ok {
            out.push((
                module,
                struct_codec_item_with(ast, module, &in_name, &in_fields(op), CodecVariant::Xcdr1)?,
            ));
        }
        in_ok.push(ok);
        out.push((module, rust::Item::Struct(out_struct(ast, service, op)?)));
        let out_name = interface::out_struct_name(&service.name, &op.name);
        out.push((
            module,
            struct_codec_item(ast, module, &out_name, &out_fields(op))?,
        ));
        let out_ok = fields_closed(ast, &out_fields(op), capable);
        if out_ok {
            out.push((
                module,
                struct_codec_item_with(
                    ast,
                    module,
                    &out_name,
                    &out_fields(op),
                    CodecVariant::Xcdr1,
                )?,
            ));
        }
        let raised_ok = op
            .raises
            .iter()
            .all(|raise| exception_ok.iter().any(|(id, ok)| *id == raise.id && *ok));
        result_ok.push(out_ok && raised_ok);
    }

    for (op, ok) in ops.iter().zip(&result_ok) {
        let (item, codec) = result_union(ast, module, service, op)?;
        out.push((module, item));
        out.push((module, codec));
        if *ok {
            out.push((
                module,
                union_codec_with(
                    &interface::result_union_name(&service.name, &op.name),
                    &result_union_cases(ast, module, service, op)?,
                    None,
                    CodecVariant::Xcdr1,
                )?,
            ));
        }
    }

    let (call_item, call_codec) = call_union(service, &ops)?;
    out.push((module, call_item));
    out.push((module, call_codec));
    let call_ok = in_ok.iter().all(|ok| *ok);
    if call_ok {
        out.push((
            module,
            union_codec_with(
                &interface::call_union_name(&service.name),
                &call_union_cases(service, &ops),
                Some(UNKNOWN_OP_VARIANT),
                CodecVariant::Xcdr1,
            )?,
        ));
    }

    let (return_item, return_codec) = return_union(service, &ops)?;
    out.push((module, return_item));
    out.push((module, return_codec));
    let return_ok = ops.iter().zip(&result_ok).all(|(_, ok)| *ok);
    if return_ok {
        out.push((
            module,
            union_codec_with(
                &interface::return_union_name(&service.name),
                &return_union_cases(service, &ops),
                Some(UNKNOWN_OP_VARIANT),
                CodecVariant::Xcdr1,
            )?,
        ));
    }

    out.push((module, rust::Item::Struct(request_struct(service))));
    out.push((module, rust::Item::Struct(reply_struct(service))));

    let exceptions = exceptions_of(&ops);
    out.push((
        module,
        rust::Item::Enum(error_enum(ast, module, service, &exceptions)?),
    ));

    // `zerodds_rpc::requester::Requester<TIn, TOut>` and
    // `zerodds_rpc::replier::Replier<TIn, TOut>` both require
    // `TIn/TOut: DdsType` — the two types they are instantiated with
    // here are `{I}_Call` and `{I}_Return`, never the per-operation
    // payloads, which reach the wire only nested inside one of those
    // two.
    out.push((
        module,
        dds_type_impl(
            &interface::call_union_name(&service.name),
            &interface::call_union_name(&service.name),
            call_ok,
        ),
    ));
    out.push((
        module,
        dds_type_impl(
            &interface::return_union_name(&service.name),
            &interface::return_union_name(&service.name),
            return_ok,
        ),
    ));

    if service.is_remote() {
        out.push((module, rust::Item::Trait(service_trait(ast, service)?)));
        out.push((module, rust::Item::Struct(client_struct(ast, service)?)));
        out.push((module, client_impl(ast, service)?));
        out.push((module, rust::Item::Struct(dispatcher_struct(service))));
        for item in dispatcher_impl(ast, service)? {
            out.push((module, item));
        }
    }

    Ok(())
}

// == field description ===================================================

/// One field of a synthesized structure: either a real IDL type, or one
/// of the three placeholder octets §7.5.1.1.1 declares
/// (`UnusedMember`/`UnknownOperation`), which have no `Id<TypeNode>` to
/// read a writer off.
enum FieldSpec {
    Real(String, Id<TypeNode>),
    Octet(String),
}

fn in_fields(op: &Operation) -> Vec<FieldSpec> {
    in_fields_styled(op, interface::EmptyOperationStyle::DummyMember)
}

/// [`in_fields`] with the Enhanced mapping's choice for an operation that
/// has nothing to carry (§7.5.1.2.1.3): the `dummy` member, or no member.
fn in_fields_styled(op: &Operation, empty: interface::EmptyOperationStyle) -> Vec<FieldSpec> {
    let mut out: Vec<FieldSpec> = op
        .inputs()
        .map(|p| FieldSpec::Real(rust::escape_keyword(&p.name), p.type_))
        .collect();
    if out.is_empty() && empty == interface::EmptyOperationStyle::DummyMember {
        out.push(FieldSpec::Octet(interface::DUMMY_MEMBER.to_string()));
    }
    out
}

fn out_fields(op: &Operation) -> Vec<FieldSpec> {
    out_fields_styled(op, interface::EmptyOperationStyle::DummyMember)
}

/// [`out_fields`] with the Enhanced mapping's choice for an operation that
/// has nothing to carry.
fn out_fields_styled(op: &Operation, empty: interface::EmptyOperationStyle) -> Vec<FieldSpec> {
    let mut out: Vec<FieldSpec> = op
        .outputs()
        .map(|p| FieldSpec::Real(rust::escape_keyword(&p.name), p.type_))
        .collect();
    if let Some(returns) = op.returns {
        let taken: Vec<&str> = out
            .iter()
            .map(|f| match f {
                FieldSpec::Real(name, _) | FieldSpec::Octet(name) => name.as_str(),
            })
            .collect();
        out.push(FieldSpec::Real(return_member_name(&taken), returns));
    }
    if out.is_empty() && empty == interface::EmptyOperationStyle::DummyMember {
        out.push(FieldSpec::Octet(interface::DUMMY_MEMBER.to_string()));
    }
    out
}

/// DDS-RPC 1.0 §7.5.1.1.5 rule 3b's collision rule, restated for Rust:
/// an `out` parameter already named `return_` pushes the return value
/// to `return_1`, `return_2`, ….
fn return_member_name(taken: &[&str]) -> String {
    if !taken.contains(&RETURN_MEMBER) {
        return RETURN_MEMBER.to_string();
    }
    for n in 1..=i32::MAX {
        let candidate = format!("{RETURN_MEMBER}{n}");
        if !taken.contains(&candidate.as_str()) {
            return candidate;
        }
    }
    format!("{RETURN_MEMBER}_")
}

fn field(name: &str, type_: rust::Type, doc: &str) -> rust::Field {
    rust::Field {
        doc: Doc::line(doc),
        attrs: Vec::new(),
        visibility: Visibility::Public,
        name: Ident::new(name),
        type_,
    }
}

/// `#[allow(non_camel_case_types)]` — every wire-named type this module
/// declares (`{I}_{op}_In`, `{I}_Call`, …) carries the spelling
/// DDS-RPC 1.0 §7.5.1.1.4-7 fixes, not Rust's naming convention; the
/// generated file answers the lint itself rather than asking a
/// consumer to.
fn allow_non_camel_case() -> rust::Attr {
    rust::Attr {
        path: Path::single("allow"),
        args: vec![rust::AttrArg::Word(Ident::new("non_camel_case_types"))],
    }
}

fn rust_field_type(
    ast: &Ast,
    from: Id<ModuleNode>,
    spec: &FieldSpec,
) -> Result<rust::Type, RustInterfaceError> {
    Ok(match spec {
        FieldSpec::Real(_, target) => rust_type_of(ast, from, Some(*target))?,
        FieldSpec::Octet(_) => rust::Type::Primitive(rust::Primitive::U8),
    })
}

fn struct_of(name: &str, fields: Vec<rust::Field>, doc: Doc) -> rust::Struct {
    rust::Struct {
        body_text: zerodds_lang_cst::common::BodyText::none(),
        doc,
        attrs: vec![allow_non_camel_case()],
        derives: data_derives(),
        visibility: Visibility::Public,
        name: Ident::new(name),
        generics: Vec::new(),
        body: rust::StructBody::Named(fields),
    }
}

fn in_struct(
    ast: &Ast,
    service: &Service,
    op: &Operation,
) -> Result<rust::Struct, RustInterfaceError> {
    let mut fields = Vec::new();
    for spec in in_fields_styled(op, service.mapping.empty_style()) {
        let type_ = rust_field_type(ast, service.module, &spec)?;
        let name = match &spec {
            FieldSpec::Real(name, _) | FieldSpec::Octet(name) => name.clone(),
        };
        let doc = field_doc(&spec);
        fields.push(field(&name, type_, &doc));
    }
    Ok(struct_of(
        &interface::in_struct_name(&service.name, &op.name),
        fields,
        Doc::line("The `in` and `inout` parameters of one call. DDS-RPC 1.0 §7.5.1.1.4."),
    ))
}

/// The doc one field of a synthesized structure carries — the IDL
/// parameter it came from, or the placeholder rule that put it there.
fn field_doc(spec: &FieldSpec) -> String {
    match spec {
        FieldSpec::Real(name, _) => format!("The `{name}` parameter."),
        FieldSpec::Octet(name) => format!(
            "`{name}`: the operation carries nothing else here. DDS-RPC 1.0 §7.5.1.1.4 rule 3c / \
             §7.5.1.1.5 rule 3c."
        ),
    }
}

fn out_struct(
    ast: &Ast,
    service: &Service,
    op: &Operation,
) -> Result<rust::Struct, RustInterfaceError> {
    let mut fields = Vec::new();
    for spec in out_fields_styled(op, service.mapping.empty_style()) {
        let type_ = rust_field_type(ast, service.module, &spec)?;
        let name = match &spec {
            FieldSpec::Real(name, _) | FieldSpec::Octet(name) => name.clone(),
        };
        let doc = if name == RETURN_MEMBER || name.starts_with(RETURN_MEMBER) {
            "The operation's return value. DDS-RPC 1.0 §7.5.1.1.5 rule 3b.".to_string()
        } else {
            field_doc(&spec)
        };
        fields.push(field(&name, type_, &doc));
    }
    Ok(struct_of(
        &interface::out_struct_name(&service.name, &op.name),
        fields,
        Doc::line(
            "The return value and the `out`/`inout` parameters of one call. DDS-RPC 1.0 §7.5.1.1.5.",
        ),
    ))
}

fn request_struct(service: &Service) -> rust::Struct {
    struct_of(
        &interface::request_type_name(&service.name),
        vec![
            field(
                "header",
                rust::Type::named(Path::new(&["zerodds_rpc", "common_types", "RequestHeader"])),
                "The request header. DDS-RPC 1.0 §7.5.1.1.6 rule 2.",
            ),
            field(
                interface::DATA_MEMBER,
                rust::Type::named(Path::single(&interface::call_union_name(&service.name))),
                "The call, as a union over every operation. DDS-RPC 1.0 §7.5.1.1.6 rule 2.",
            ),
        ],
        Doc::lines(&[
            "The request topic type. DDS-RPC 1.0 §7.5.1.1.6.",
            "",
            "Declared for wire-shape parity with the C++ and Java bindings; \
             `zerodds_rpc::requester::Requester` frames the header itself \
             (`crates/rpc/src/wire_codec.rs`) and takes the `data` type — \
             this interface's `_Call`/`_Return` — as its own two type \
             parameters, so this type is never constructed by the \
             generated client or dispatcher.",
        ]),
    )
}

fn reply_struct(service: &Service) -> rust::Struct {
    struct_of(
        &interface::reply_type_name(&service.name),
        vec![
            field(
                "header",
                rust::Type::named(Path::new(&["zerodds_rpc", "common_types", "ReplyHeader"])),
                "The reply header. DDS-RPC 1.0 §7.5.1.1.7 rule 3.",
            ),
            field(
                interface::DATA_MEMBER,
                rust::Type::named(Path::single(&interface::return_union_name(&service.name))),
                "The outcome, as a union over every non-`oneway` operation. \
                 DDS-RPC 1.0 §7.5.1.1.7 rule 3.",
            ),
        ],
        Doc::line("The reply topic type. DDS-RPC 1.0 §7.5.1.1.7. See `Request`'s own doc."),
    )
}

// == codec ================================================================

fn position_for(ast: &Ast, target: Id<TypeNode>) -> Position {
    Position {
        behaviour: ast.effective_try_construct(None, Some(target)),
        written: None,
    }
}

fn member_write_with(
    ast: &Ast,
    from: Id<ModuleNode>,
    target: Id<TypeNode>,
    variant: CodecVariant,
) -> MemberWrite {
    write_of(
        ast,
        from,
        target,
        variant.key(),
        variant.suffix(),
        position_for(ast, target),
    )
}

/// The length code of the `EMHEADER` of a member of type `target`
/// (XTypes 1.3 §7.4.3.4.2) in the Enhanced Service Mapping's PL_CDR2 form.
///
/// The Rust codec decides it from the member's write form; the C++ and the
/// Java codec of the same structures ask here, so the three write the same
/// octets.
pub(crate) fn emheader_length_code(ast: &Ast, from: Id<ModuleNode>, target: Id<TypeNode>) -> u8 {
    enhanced::length_code(&member_write_with(ast, from, target, CodecVariant::Xcdr2))
}

fn self_field(name: &str) -> rust::Expr {
    rust::Expr::Field {
        receiver: Box::new(rust::Expr::Path(Path::single("self"))),
        name: Ident::new(name),
    }
}

/// One structure's `encode`/`decode`, member by member, through
/// [`write_value_to`] and [`read_for`] — the same two functions
/// `crate::encode::rust_codec` calls for a declared `struct`. `@final`
/// throughout: no `DHEADER`, matching the module doc.
fn struct_codec_item(
    ast: &Ast,
    from: Id<ModuleNode>,
    type_name: &str,
    fields: &[FieldSpec],
) -> Result<rust::Item, RustInterfaceError> {
    struct_codec_item_with(ast, from, type_name, fields, CodecVariant::Xcdr2)
}

/// [`struct_codec_item`] for one of the two codec pairs.
fn struct_codec_item_with(
    ast: &Ast,
    from: Id<ModuleNode>,
    type_name: &str,
    fields: &[FieldSpec],
    variant: CodecVariant,
) -> Result<rust::Item, RustInterfaceError> {
    let mut encode_statements = Vec::new();
    let mut decode_statements = Vec::new();
    let mut struct_fields = Vec::new();
    for spec in fields {
        let (name, kind) = match spec {
            FieldSpec::Real(name, target) => {
                (name.clone(), member_write_with(ast, from, *target, variant))
            }
            FieldSpec::Octet(name) => (name.clone(), MemberWrite::Primitive(PrimitiveKind::Octet)),
        };
        encode_statements.push(write_value_to("w", &name, &kind, self_field(&name), false)?);
        decode_statements.push(rust::Statement::Let {
            name: Ident::new(&name),
            type_: None,
            value: Some(rust::Expr::Try(Box::new(read_for(&name, &kind)?))),
            mutable: false,
        });
        struct_fields.push((Ident::new(&name), rust::Expr::Path(Path::single(&name))));
    }
    if fields.is_empty() {
        // An `@Empty` structure (§7.5.1.2.1.3) writes and reads nothing; the
        // writer and the reader are still parameters.
        for (statements, name) in [(&mut encode_statements, "w"), (&mut decode_statements, "r")] {
            statements.push(rust::Statement::Let {
                name: Ident::new("_"),
                type_: None,
                value: Some(rust::Expr::Ref {
                    target: Box::new(rust::Expr::Path(Path::single(name))),
                    mutable: false,
                }),
                mutable: false,
            });
        }
    }
    encode_statements.push(rust::Statement::Tail(ok_unit()));
    decode_statements.push(rust::Statement::Tail(rust::Expr::Call {
        callee: Box::new(rust::Expr::Path(Path::single("Ok"))),
        args: vec![rust::Expr::StructLit {
            path: Path::single("Self"),
            fields: struct_fields,
            base: None,
        }],
    }));
    Ok(rust::Item::Impl(rust::Impl {
        doc: Doc::none(),
        attrs: Vec::new(),
        generics: Vec::new(),
        trait_: None,
        self_type: rust::Type::named(Path::single(type_name)),
        items: vec![
            encode_fn_item_named(&variant.encode_name(), type_name, encode_statements),
            decode_fn_item_named(&variant.decode_name(), type_name, decode_statements),
        ],
    }))
}

fn ok_unit() -> rust::Expr {
    rust::Expr::Call {
        callee: Box::new(rust::Expr::Path(Path::single("Ok"))),
        args: vec![rust::Expr::Tuple(Vec::new())],
    }
}

fn writer_ref_type() -> rust::Type {
    rust::Type::Ref {
        target: Box::new(rust::Type::named(Path::single("Writer"))),
        mutable: true,
        lifetime: None,
    }
}

fn reader_ref_type() -> rust::Type {
    rust::Type::Ref {
        target: Box::new(rust::Type::named(Path::single("Reader"))),
        mutable: true,
        lifetime: None,
    }
}

fn wire_error_result(ok: rust::Type) -> rust::Type {
    rust::Type::generic(
        Path::single("Result"),
        vec![ok, rust::Type::named(Path::single("WireError"))],
    )
}

fn encode_fn_item_named(
    name: &str,
    type_name: &str,
    statements: Vec<rust::Statement>,
) -> rust::Item {
    rust::Item::Function(rust::Function {
        doc: Doc::line(&format!(
            "Writes `{type_name}` to `w` with `crate::encode::rust_codec`'s CDR encoding."
        )),
        attrs: Vec::new(),
        visibility: Visibility::Public,
        name: Ident::new(name),
        generics: Vec::new(),
        params: vec![
            rust::Param {
                name: None,
                type_: rust::Type::unit(),
                receiver: Some(rust::Receiver::Ref),
            },
            rust::Param {
                name: Some(Ident::new("w")),
                type_: writer_ref_type(),
                receiver: None,
            },
        ],
        returns: Some(wire_error_result(rust::Type::unit())),
        body: Some(rust::Block { statements }),
    })
}

fn decode_fn_item_named(
    name: &str,
    type_name: &str,
    statements: Vec<rust::Statement>,
) -> rust::Item {
    rust::Item::Function(rust::Function {
        doc: Doc::line(&format!(
            "Reads a `{type_name}` from `r` with `crate::encode::rust_codec`'s CDR decoding."
        )),
        attrs: Vec::new(),
        visibility: Visibility::Public,
        name: Ident::new(name),
        generics: Vec::new(),
        params: vec![rust::Param {
            name: Some(Ident::new("r")),
            type_: reader_ref_type(),
            receiver: None,
        }],
        returns: Some(wire_error_result(rust::Type::named(Path::single("Self")))),
        body: Some(rust::Block { statements }),
    })
}

// -- exceptions -----------------------------------------------------------

fn exceptions_of(ops: &[Operation]) -> Vec<Raise> {
    let mut out: Vec<Raise> = Vec::new();
    for op in ops {
        for raise in &op.raises {
            if !out.iter().any(|seen| seen.id == raise.id) {
                out.push(raise.clone());
            }
        }
    }
    out
}

fn exception_module(
    ast: &Ast,
    id: Id<ExceptionNode>,
) -> Result<Id<ModuleNode>, RustInterfaceError> {
    let node = ast.exception(id).map_err(|_| InterfaceError::NoNode {
        kind: "exception",
        at: "the model".to_string(),
    })?;
    Ok(node.module)
}

/// The path to an exception's already-declared Rust struct, relative to
/// `from`. `crate::adapter::cst::relative_path` cannot be reused
/// directly — it takes a `&TypeNode` and an exception is not one — so
/// this walks the same shared-prefix logic over
/// `adapter::interface::module_chain`.
fn exception_path(
    ast: &Ast,
    from: Id<ModuleNode>,
    id: Id<ExceptionNode>,
) -> Result<Path, RustInterfaceError> {
    let node = ast.exception(id).map_err(|_| InterfaceError::NoNode {
        kind: "exception",
        at: "the model".to_string(),
    })?;
    relative_module_path(ast, from, node.module, &node.name)
}

/// A declared type's path as written from inside module `from`, given
/// only the module it sits in and its own name.
///
/// `crate::adapter::cst::relative_path` cannot be reused directly — it
/// takes a `&TypeNode`, and neither an exception nor an interface (an
/// RPC trait's base) is one. Same logic, over
/// `adapter::interface::module_chain`.
fn relative_module_path(
    ast: &Ast,
    from: Id<ModuleNode>,
    target_module: Id<ModuleNode>,
    leaf: &str,
) -> Result<Path, RustInterfaceError> {
    let here = interface::module_chain(ast, from)?;
    let there = interface::module_chain(ast, target_module)?;
    let shared = here
        .iter()
        .zip(there.iter())
        .take_while(|(a, b)| a.0 == b.0)
        .count();
    let mut segments: Vec<String> = Vec::new();
    for _ in shared..here.len() {
        segments.push("super".to_string());
    }
    for (_, name) in there.iter().skip(shared) {
        segments.push(rust::escape_keyword(name));
    }
    segments.push(rust::escape_keyword(leaf));
    let borrowed: Vec<&str> = segments.iter().map(String::as_str).collect();
    Ok(Path::new(&borrowed))
}

fn exception_codec_item(
    ast: &Ast,
    id: Id<ExceptionNode>,
) -> Result<rust::Item, RustInterfaceError> {
    exception_codec_item_with(ast, id, CodecVariant::Xcdr2)
}

/// Whether every declared type an exception's members reach has an XCDR1
/// pair.
fn exception_fields_closed(
    ast: &Ast,
    id: Id<ExceptionNode>,
    capable: &BTreeSet<Id<TypeNode>>,
) -> Result<bool, RustInterfaceError> {
    let node = ast.exception(id).map_err(|_| InterfaceError::NoNode {
        kind: "exception",
        at: "the model".to_string(),
    })?;
    Ok(node.members.iter().all(|member| {
        ast.member(*member).is_ok_and(|member| {
            member
                .type_
                .resolved()
                .is_some_and(|target| type_closed(ast, target, capable, &mut BTreeSet::new()))
        })
    }))
}

fn exception_codec_item_with(
    ast: &Ast,
    id: Id<ExceptionNode>,
    variant: CodecVariant,
) -> Result<rust::Item, RustInterfaceError> {
    let node = ast.exception(id).map_err(|_| InterfaceError::NoNode {
        kind: "exception",
        at: "the model".to_string(),
    })?;
    let module = node.module;
    let mut fields = Vec::new();
    for member in &node.members {
        let member_node = ast.member(*member).map_err(|_| InterfaceError::NoNode {
            kind: "parameter",
            at: node.name.clone(),
        })?;
        let Some(target) = member_node.type_.resolved() else {
            return Err(InterfaceError::Unresolved {
                construct: "exception member type",
                at: format!("{}::{}", node.name, member_node.name),
            }
            .into());
        };
        fields.push(FieldSpec::Real(
            rust::escape_keyword(&member_node.name),
            target,
        ));
    }
    struct_codec_item_with(ast, module, &node.name, &fields, variant)
}

// == tagged unions =========================================================

/// One case of a tagged union: the enum variant name, the path to the
/// type its payload carries (already declared, with its own
/// `encode`/`decode`), the `long` discriminator DDS-RPC 1.0 assigns it,
/// and a doc line.
///
/// The payload is a [`Path`] and not a [`rust::Type`]: every case here
/// is a named struct or a named union, never a primitive or a
/// collection, and keeping the path lets [`decode_case_body`] build
/// `<Path>::decode(r)` without taking a named type apart again to find
/// out what it already is.
struct UnionCase {
    variant: String,
    payload: Path,
    discriminator: i32,
    doc: String,
}

/// `${I}_Call`, `${I}_Return` or `${I}_${op}_Result` as a Rust `enum`,
/// plus its `encode`/`decode`.
///
/// `default_variant` is `Some(name)` for the `unknownOp` case DDS-RPC
/// 1.0 §7.5.1.1.6 rule 3 and §7.5.1.1.7 rule 3 both require on the
/// `Call` and `Return` unions; `None` for the per-operation `Result`
/// union of §7.5.1.1.5, which has no such case — an unrecognised
/// discriminator there is refused on decode rather than defaulted,
/// because there is no spec-given value to construct.
fn tagged_union(
    name: &str,
    doc: Doc,
    cases: Vec<UnionCase>,
    default_variant: Option<&str>,
) -> Result<(rust::Item, rust::Item), RustInterfaceError> {
    let mut variants = Vec::new();
    for case in &cases {
        variants.push(rust::Variant {
            doc: Doc::line(&case.doc),
            attrs: Vec::new(),
            name: Ident::new(&case.variant),
            body: rust::VariantBody::Tuple(vec![rust::Type::named(case.payload.clone())]),
            discriminant: None,
        });
    }
    if let Some(default_variant) = default_variant {
        variants.push(rust::Variant {
            doc: Doc::line(
                "The default case: a discriminator this binding does not recognise. \
                 DDS-RPC 1.0 §7.5.1.1.6 rule 3 / §7.5.1.1.7 rule 3.",
            ),
            attrs: Vec::new(),
            name: Ident::new(default_variant),
            body: rust::VariantBody::Tuple(vec![rust::Type::Primitive(rust::Primitive::U8)]),
            discriminant: None,
        });
    }
    let item = rust::Item::Enum(rust::Enum {
        doc,
        attrs: vec![allow_non_camel_case()],
        derives: data_derives(),
        visibility: Visibility::Public,
        name: Ident::new(name),
        generics: Vec::new(),
        variants,
    });

    let codec = union_codec_with(name, &cases, default_variant, CodecVariant::Xcdr2)?;
    Ok((item, codec))
}

/// The `encode`/`decode` pair of a tagged union for one of the two codec
/// pairs: the `long` discriminator, then the case payload's own pair of
/// the same version.
fn union_codec_with(
    name: &str,
    cases: &[UnionCase],
    default_variant: Option<&str>,
    variant: CodecVariant,
) -> Result<rust::Item, RustInterfaceError> {
    let mut encode_arms = Vec::new();
    let mut decode_arms = Vec::new();
    for case in cases {
        encode_arms.push(rust::MatchArm {
            pattern: rust::Pattern::TupleStruct {
                path: Path::new(&["Self", &case.variant]),
                elements: vec![rust::Pattern::Binding(Ident::new("value"))],
            },
            guard: None,
            body: encode_case_body(case.discriminator, "value", variant)?,
        });
        decode_arms.push(rust::MatchArm {
            pattern: rust::Pattern::Literal(Literal::int(i128::from(case.discriminator))),
            guard: None,
            body: decode_case_body(&case.variant, &case.payload, variant),
        });
    }
    if let Some(default_variant) = default_variant {
        encode_arms.push(rust::MatchArm {
            pattern: rust::Pattern::TupleStruct {
                path: Path::new(&["Self", default_variant]),
                elements: vec![rust::Pattern::Binding(Ident::new("value"))],
            },
            guard: None,
            body: encode_default_body("value")?,
        });
        decode_arms.push(rust::MatchArm {
            pattern: rust::Pattern::Wildcard,
            guard: None,
            body: decode_default_body(default_variant)?,
        });
    } else {
        decode_arms.push(rust::MatchArm {
            pattern: rust::Pattern::Wildcard,
            guard: None,
            body: refused_expr("an unrecognised DDS-RPC 1.0 §7.5.1.1.5 Result discriminator"),
        });
    }

    let encode_statements = vec![rust::Statement::Tail(rust::Expr::Match {
        scrutinee: Box::new(rust::Expr::Path(Path::single("self"))),
        arms: encode_arms,
    })];
    let decode_statements = vec![
        rust::Statement::Let {
            name: Ident::new("disc"),
            type_: Some(rust::Type::Primitive(rust::Primitive::I32)),
            value: Some(rust::Expr::Try(Box::new(read_discriminator()?))),
            mutable: false,
        },
        rust::Statement::Tail(rust::Expr::Match {
            scrutinee: Box::new(rust::Expr::Path(Path::single("disc"))),
            arms: decode_arms,
        }),
    ];
    let codec = rust::Item::Impl(rust::Impl {
        doc: Doc::none(),
        attrs: Vec::new(),
        generics: Vec::new(),
        trait_: None,
        self_type: rust::Type::named(Path::single(name)),
        items: vec![
            encode_fn_item_named(&variant.encode_name(), name, encode_statements),
            decode_fn_item_named(&variant.decode_name(), name, decode_statements),
        ],
    });
    Ok(codec)
}

/// `w.write_i32(&value)?` in shape but never fallible in practice: an
/// `Int32` write never refuses. [`write_value_to`] is still called
/// through — not hand-assembled — so a change to how the runtime writes
/// an `Int32` reaches the discriminator the same call reaches every
/// other `Int32` member through, and the `EncodeError` it could in
/// principle answer with is reported rather than assumed away.
fn discriminator_write(value: rust::Expr) -> Result<rust::Statement, RustInterfaceError> {
    Ok(write_value_to(
        "w",
        "_d",
        &MemberWrite::Primitive(PrimitiveKind::Int32),
        value,
        false,
    )?)
}

fn read_discriminator() -> Result<rust::Expr, RustInterfaceError> {
    Ok(read_for(
        "_d",
        &MemberWrite::Primitive(PrimitiveKind::Int32),
    )?)
}

fn encode_case_body(
    discriminator: i32,
    binding: &str,
    variant: CodecVariant,
) -> Result<rust::Expr, RustInterfaceError> {
    Ok(rust::Expr::Block(rust::Block {
        statements: vec![
            discriminator_write(rust::Expr::Literal(Literal::int(i128::from(discriminator))))?,
            rust::Statement::Tail(rust::Expr::MethodCall {
                receiver: Box::new(rust::Expr::Path(Path::single(binding))),
                method: Ident::new(&variant.encode_name()),
                args: vec![rust::Expr::Path(Path::single("w"))],
            }),
        ],
    }))
}

fn encode_default_body(binding: &str) -> Result<rust::Expr, RustInterfaceError> {
    Ok(rust::Expr::Block(rust::Block {
        statements: vec![
            discriminator_write(rust::Expr::Literal(Literal::int(0)))?,
            write_value_to(
                "w",
                "unknown_op",
                &MemberWrite::Primitive(PrimitiveKind::Octet),
                rust::Expr::Path(Path::single(binding)),
                true,
            )?,
            rust::Statement::Tail(ok_unit()),
        ],
    }))
}

fn decode_case_body(variant: &str, payload: &Path, codec: CodecVariant) -> rust::Expr {
    rust::Expr::Call {
        callee: Box::new(rust::Expr::Path(Path::single("Ok"))),
        args: vec![rust::Expr::Call {
            callee: Box::new(rust::Expr::Path(Path::new(&["Self", variant]))),
            args: vec![rust::Expr::Try(Box::new(rust::Expr::Call {
                callee: Box::new(rust::Expr::Path(payload.join(&codec.decode_name()))),
                args: vec![rust::Expr::Path(Path::single("r"))],
            }))],
        }],
    }
}

fn decode_default_body(default_variant: &str) -> Result<rust::Expr, RustInterfaceError> {
    Ok(rust::Expr::Call {
        callee: Box::new(rust::Expr::Path(Path::single("Ok"))),
        args: vec![rust::Expr::Call {
            callee: Box::new(rust::Expr::Path(Path::new(&["Self", default_variant]))),
            args: vec![rust::Expr::Try(Box::new(read_for(
                "unknown_op",
                &MemberWrite::Primitive(PrimitiveKind::Octet),
            )?))],
        }],
    })
}

fn refused_expr(why: &str) -> rust::Expr {
    rust::Expr::Call {
        callee: Box::new(rust::Expr::Path(Path::single("Err"))),
        args: vec![rust::Expr::Call {
            callee: Box::new(rust::Expr::Path(Path::new(&["WireError", "refused"]))),
            args: vec![
                rust::Expr::MethodCall {
                    receiver: Box::new(rust::Expr::Path(Path::single("r"))),
                    method: Ident::new("position"),
                    args: Vec::new(),
                },
                rust::Expr::Literal(Literal::string(why)),
            ],
        }],
    }
}

// -- Result, Call, Return --------------------------------------------------

fn result_union(
    ast: &Ast,
    module: Id<ModuleNode>,
    service: &Service,
    op: &Operation,
) -> Result<(rust::Item, rust::Item), RustInterfaceError> {
    let cases = result_union_cases(ast, module, service, op)?;
    let name = interface::result_union_name(&service.name, &op.name);
    tagged_union(
        &name,
        Doc::line(&format!(
            "The outcome of `{}`. DDS-RPC 1.0 §7.5.1.1.5.",
            op.name
        )),
        cases,
        None,
    )
}

/// The cases of `${I}_${op}_Result`.
fn result_union_cases(
    ast: &Ast,
    module: Id<ModuleNode>,
    service: &Service,
    op: &Operation,
) -> Result<Vec<UnionCase>, RustInterfaceError> {
    let mut cases = vec![UnionCase {
        variant: "Ok".to_string(),
        payload: Path::single(&interface::out_struct_name(&service.name, &op.name)),
        discriminator: interface::RETCODE_OK,
        doc: "`dds::RETCODE_OK`. DDS-RPC 1.0 §7.5.1.1.5 rule 3a.".to_string(),
    }];
    for raise in &op.raises {
        cases.push(UnionCase {
            variant: rust::escape_keyword(&raise.name),
            payload: exception_path(ast, module, raise.id)?,
            discriminator: interface::exception_hash(&raise.scoped_name),
            doc: format!("The `{}` case. DDS-RPC 1.0 §7.5.1.1.5 rule 5.", raise.name),
        });
    }
    Ok(cases)
}

fn exception_type(
    ast: &Ast,
    from: Id<ModuleNode>,
    id: Id<ExceptionNode>,
) -> Result<rust::Type, RustInterfaceError> {
    Ok(rust::Type::named(exception_path(ast, from, id)?))
}

fn call_union(
    service: &Service,
    ops: &[Operation],
) -> Result<(rust::Item, rust::Item), RustInterfaceError> {
    let cases = call_union_cases(service, ops);
    tagged_union(
        &interface::call_union_name(&service.name),
        Doc::line("The request union over every operation. DDS-RPC 1.0 §7.5.1.1.6."),
        cases,
        Some(UNKNOWN_OP_VARIANT),
    )
}

/// The cases of `${I}_Call`.
fn call_union_cases(service: &Service, ops: &[Operation]) -> Vec<UnionCase> {
    ops.iter()
        .map(|op| UnionCase {
            variant: variant_name(&op.name),
            payload: Path::single(&interface::in_struct_name(&service.name, &op.name)),
            discriminator: interface::operation_hash(&op.name),
            doc: format!("`{}`. DDS-RPC 1.0 §7.5.1.1.6 rule 5.", op.name),
        })
        .collect()
}

fn return_union(
    service: &Service,
    ops: &[Operation],
) -> Result<(rust::Item, rust::Item), RustInterfaceError> {
    let cases = return_union_cases(service, ops);
    tagged_union(
        &interface::return_union_name(&service.name),
        Doc::line("The reply union. DDS-RPC 1.0 §7.5.1.1.7."),
        cases,
        Some(UNKNOWN_OP_VARIANT),
    )
}

/// The cases of `${I}_Return`.
fn return_union_cases(service: &Service, ops: &[Operation]) -> Vec<UnionCase> {
    ops.iter()
        .map(|op| UnionCase {
            variant: variant_name(&op.name),
            payload: Path::single(&interface::result_union_name(&service.name, &op.name)),
            discriminator: interface::operation_hash(&op.name),
            doc: format!("`{}`. DDS-RPC 1.0 §7.5.1.1.7 rule 5b.", op.name),
        })
        .collect()
}

/// The enum variant DDS-RPC 1.0's `unknownOp` default case takes in
/// this binding. `crate::adapter::interface::UNKNOWN_OP_MEMBER` is the
/// wire member name the C++ and Java bindings spell; this is its Rust
/// variant spelling, named once so the `Call` and `Return` unions agree
/// with each other and with `dispatcher_impl`.
const UNKNOWN_OP_VARIANT: &str = "UnknownOp";

/// The variant an Enhanced `Result` choice holds a member id it does not
/// know in: an exception member added by a newer version of the
/// interface (DDS-RPC 1.0 §7.7.2.5.6).
const UNKNOWN_EX_VARIANT: &str = "UnknownEx";

/// A PascalCase enum variant from an IDL name. `get_memory` becomes
/// `GetMemory`; a name that is already one word capitalises its first
/// letter and keeps the rest, so `add` becomes `Add`.
fn variant_name(name: &str) -> String {
    let mut out = String::new();
    for segment in name.split('_') {
        let mut chars = segment.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.extend(chars);
        }
    }
    if out.is_empty() {
        out.push_str("Op");
    }
    out
}

fn field_expr(receiver: &str, field_name: &str) -> rust::Expr {
    rust::Expr::Field {
        receiver: Box::new(rust::Expr::Path(Path::single(receiver))),
        name: Ident::new(field_name),
    }
}

fn field_vis(name: &str, type_: rust::Type, visibility: Visibility, doc: &str) -> rust::Field {
    rust::Field {
        doc: Doc::line(doc),
        attrs: Vec::new(),
        visibility,
        name: Ident::new(name),
        type_,
    }
}

fn error_type_name(service: &Service) -> String {
    format!("{}Error", service.name)
}

// == DdsType bridge ========================================================

/// `impl zerodds_dcps::dds_type::DdsType for {type_name}`, over the
/// inherent `encode`/`decode` [`struct_codec_item`] and [`tagged_union`]
/// both already give every synthesized type — the same bridge
/// `crates/zerodds-build/src/dds_type.rs` writes for a declared
/// `struct`, at the one XCDR2-little-endian cell
/// `zerodds_rpc::requester::Requester::send_request_blocking` reads
/// (`TIn::encode`, never `encode_be`), so only the trait's two required
/// methods are given — every other one keeps the trait's own refusing
/// default.
/// A `DdsType` impl for a type declared as `type_name` and named in DDS
/// `dds_name`: a structure in an IDL module is declared by its simple name
/// and named in DDS by its scoped one.
pub(crate) fn dds_type_impl(type_name: &str, dds_name: &str, xcdr1: bool) -> rust::Item {
    let type_name_const = rust::Item::Const(rust::Const {
        doc: Doc::line(
            "Informational only: `zerodds_rpc::requester::Requester` never \
             registers this as a DDS topic type — it frames the request/reply \
             header itself and writes the sample through `DdsType::encode` \
             directly. See `crates/rpc/src/wire_codec.rs`.",
        ),
        attrs: Vec::new(),
        visibility: Visibility::Private,
        name: Ident::new("TYPE_NAME"),
        type_: rust::Type::Ref {
            target: Box::new(rust::Type::Primitive(rust::Primitive::Str)),
            mutable: false,
            lifetime: Some(Ident::new("static")),
        },
        value: rust::Expr::Literal(Literal::string(dds_name)),
    });

    let encode_fn = rust::Item::Function(rust::Function {
        doc: Doc::none(),
        attrs: Vec::new(),
        visibility: Visibility::Private,
        name: Ident::new("encode"),
        generics: Vec::new(),
        params: vec![
            rust::Param {
                name: None,
                type_: rust::Type::unit(),
                receiver: Some(rust::Receiver::Ref),
            },
            rust::Param {
                name: Some(Ident::new("out")),
                type_: rust::Type::Ref {
                    target: Box::new(rust::Type::generic(
                        Path::single("Vec"),
                        vec![rust::Type::Primitive(rust::Primitive::U8)],
                    )),
                    mutable: true,
                    lifetime: None,
                },
                receiver: None,
            },
        ],
        returns: Some(rust::Type::generic(
            Path::single("Result"),
            vec![
                rust::Type::unit(),
                rust::Type::named(Path::new(&["zerodds_dcps", "dds_type", "EncodeError"])),
            ],
        )),
        body: Some(rust::Block {
            statements: vec![
                rust::Statement::Let {
                    name: Ident::new("w"),
                    type_: None,
                    value: Some(rust::Expr::Call {
                        callee: Box::new(rust::Expr::Path(Path::new(&[
                            "zerodds_wire",
                            "Writer",
                            "with_max_align",
                        ]))),
                        args: vec![
                            rust::Expr::Path(Path::new(&["zerodds_wire", "Endianness", "Little"])),
                            rust::Expr::Literal(Literal::int(4)),
                        ],
                    }),
                    mutable: true,
                },
                rust::Statement::Semi(rust::Expr::Try(Box::new(rust::Expr::Call {
                    callee: Box::new(rust::Expr::Path(Path::new(&["Self", "encode"]))),
                    args: vec![
                        rust::Expr::Path(Path::single("self")),
                        rust::Expr::Ref {
                            target: Box::new(rust::Expr::Path(Path::single("w"))),
                            mutable: true,
                        },
                    ],
                }))),
                rust::Statement::Semi(rust::Expr::MethodCall {
                    receiver: Box::new(rust::Expr::Path(Path::single("out"))),
                    method: Ident::new("extend_from_slice"),
                    args: vec![rust::Expr::Ref {
                        target: Box::new(rust::Expr::MethodCall {
                            receiver: Box::new(rust::Expr::Path(Path::single("w"))),
                            method: Ident::new("finish"),
                            args: Vec::new(),
                        }),
                        mutable: false,
                    }],
                }),
                rust::Statement::Tail(ok_unit()),
            ],
        }),
    });

    let decode_fn = rust::Item::Function(rust::Function {
        doc: Doc::none(),
        attrs: Vec::new(),
        visibility: Visibility::Private,
        name: Ident::new("decode"),
        generics: Vec::new(),
        params: vec![rust::Param {
            name: Some(Ident::new("bytes")),
            type_: rust::Type::Ref {
                target: Box::new(rust::Type::Slice(Box::new(rust::Type::Primitive(
                    rust::Primitive::U8,
                )))),
                mutable: false,
                lifetime: None,
            },
            receiver: None,
        }],
        returns: Some(rust::Type::generic(
            Path::single("Result"),
            vec![
                rust::Type::named(Path::single("Self")),
                rust::Type::named(Path::new(&["zerodds_dcps", "dds_type", "DecodeError"])),
            ],
        )),
        body: Some(rust::Block {
            statements: vec![
                rust::Statement::Let {
                    name: Ident::new("r"),
                    type_: None,
                    value: Some(rust::Expr::Call {
                        callee: Box::new(rust::Expr::Path(Path::new(&[
                            "zerodds_wire",
                            "Reader",
                            "with_max_align",
                        ]))),
                        args: vec![
                            rust::Expr::Path(Path::single("bytes")),
                            rust::Expr::Path(Path::new(&["zerodds_wire", "Endianness", "Little"])),
                            rust::Expr::Literal(Literal::int(4)),
                        ],
                    }),
                    mutable: true,
                },
                rust::Statement::Tail(rust::Expr::Call {
                    callee: Box::new(rust::Expr::Path(Path::single("Ok"))),
                    args: vec![rust::Expr::Try(Box::new(rust::Expr::Call {
                        callee: Box::new(rust::Expr::Path(Path::new(&["Self", "decode"]))),
                        args: vec![rust::Expr::Ref {
                            target: Box::new(rust::Expr::Path(Path::single("r"))),
                            mutable: true,
                        }],
                    }))],
                }),
            ],
        }),
    });

    rust::Item::Impl(rust::Impl {
        doc: Doc::none(),
        attrs: Vec::new(),
        generics: Vec::new(),
        trait_: Some(rust::Type::named(Path::new(&[
            "zerodds_dcps",
            "dds_type",
            "DdsType",
        ]))),
        self_type: rust::Type::named(Path::single(type_name)),
        items: {
            let mut items = vec![type_name_const, encode_fn, decode_fn];
            // The XCDR1 methods only for a type that has the XCDR1 pair:
            // otherwise they keep the trait's refusing defaults, so a
            // Basic endpoint on classic CDR says why it cannot send.
            if xcdr1 {
                items.extend(continuing_codec_fns());
            }
            items
        },
    })
}

/// `zerodds_dcps::dds_type::DdsType::{encode_xcdr1, decode_xcdr1,
/// encode_continuing, decode_continuing}` for a topic type.
///
/// DDS-RPC 1.0 §7.5.1.1: the Basic Service Mapping targets DDS 1.3, which
/// has classic CDR (XCDR1) only, where an 8-octet member aligns to 8 and
/// not to 4. The same inherent `encode`/`decode` write both — the writer's
/// alignment cap is the whole difference for a `@final` type — and the
/// `continuing` pair starts them at the offset the type has inside the
/// sample, behind the `RequestHeader` or `ReplyHeader`, so the alignment is
/// the sample's.
fn continuing_codec_fns() -> Vec<rust::Item> {
    fn path(parts: &[&str]) -> rust::Expr {
        rust::Expr::Path(Path::new(parts))
    }
    fn call(parts: &[&str], args: Vec<rust::Expr>) -> rust::Expr {
        rust::Expr::Call {
            callee: Box::new(path(parts)),
            args,
        }
    }
    fn local(name: &str) -> rust::Expr {
        rust::Expr::Path(Path::single(name))
    }
    fn mut_ref(name: &str) -> rust::Expr {
        rust::Expr::Ref {
            target: Box::new(local(name)),
            mutable: true,
        }
    }
    fn let_(name: &str, value: rust::Expr, mutable: bool) -> rust::Statement {
        rust::Statement::Let {
            name: Ident::new(name),
            type_: None,
            value: Some(value),
            mutable,
        }
    }
    fn method(receiver: rust::Expr, name: &str, args: Vec<rust::Expr>) -> rust::Expr {
        rust::Expr::MethodCall {
            receiver: Box::new(receiver),
            method: Ident::new(name),
            args,
        }
    }
    fn lit(n: i128) -> rust::Expr {
        rust::Expr::Literal(Literal::int(n))
    }
    let little = || path(&["zerodds_wire", "Endianness", "Little"]);
    let self_param = rust::Param {
        name: None,
        type_: rust::Type::unit(),
        receiver: Some(rust::Receiver::Ref),
    };
    let bytes_param = |name: &str| rust::Param {
        name: Some(Ident::new(name)),
        type_: rust::Type::Ref {
            target: Box::new(rust::Type::Slice(Box::new(rust::Type::Primitive(
                rust::Primitive::U8,
            )))),
            mutable: false,
            lifetime: None,
        },
        receiver: None,
    };
    let vec_u8_mut = rust::Type::Ref {
        target: Box::new(rust::Type::generic(
            Path::single("Vec"),
            vec![rust::Type::Primitive(rust::Primitive::U8)],
        )),
        mutable: true,
        lifetime: None,
    };
    let bool_param = |name: &str| rust::Param {
        name: Some(Ident::new(name)),
        type_: rust::Type::Primitive(rust::Primitive::Bool),
        receiver: None,
    };
    let encode_result = || {
        rust::Type::generic(
            Path::single("Result"),
            vec![
                rust::Type::unit(),
                rust::Type::named(Path::new(&["zerodds_dcps", "dds_type", "EncodeError"])),
            ],
        )
    };
    let decode_result = || {
        rust::Type::generic(
            Path::single("Result"),
            vec![
                rust::Type::named(Path::single("Self")),
                rust::Type::named(Path::new(&["zerodds_dcps", "dds_type", "DecodeError"])),
            ],
        )
    };
    let function = |name: &str,
                    params: Vec<rust::Param>,
                    returns: rust::Type,
                    statements: Vec<rust::Statement>| {
        rust::Item::Function(rust::Function {
            doc: Doc::none(),
            attrs: Vec::new(),
            visibility: Visibility::Private,
            name: Ident::new(name),
            generics: Vec::new(),
            params,
            returns: Some(returns),
            body: Some(rust::Block { statements }),
        })
    };
    // `if xcdr1 { 8 } else { 4 }`: the alignment cap of the stream.
    let cap = || rust::Expr::If {
        condition: Box::new(local("xcdr1")),
        then: rust::Block {
            statements: vec![rust::Statement::Tail(lit(8))],
        },
        otherwise: Some(rust::Block {
            statements: vec![rust::Statement::Tail(lit(4))],
        }),
    };

    // The inherent pair of the version: `encode_xcdr1` for classic CDR,
    // `encode` for XCDR2.
    let by_version = |xcdr1_fn: &str, xcdr2_fn: &str, args: Vec<rust::Expr>| rust::Expr::If {
        condition: Box::new(local("xcdr1")),
        then: rust::Block {
            statements: vec![rust::Statement::Tail(call(
                &["Self", xcdr1_fn],
                args.clone(),
            ))],
        },
        otherwise: Some(rust::Block {
            statements: vec![rust::Statement::Tail(call(&["Self", xcdr2_fn], args))],
        }),
    };

    // encode_xcdr1
    let encode_xcdr1 = function(
        "encode_xcdr1",
        vec![
            self_param.clone(),
            rust::Param {
                name: Some(Ident::new("out")),
                type_: vec_u8_mut.clone(),
                receiver: None,
            },
        ],
        encode_result(),
        vec![
            let_(
                "w",
                call(
                    &["zerodds_wire", "Writer", "with_max_align"],
                    vec![little(), lit(8)],
                ),
                true,
            ),
            rust::Statement::Semi(rust::Expr::Try(Box::new(call(
                &["Self", "encode_xcdr1"],
                vec![local("self"), mut_ref("w")],
            )))),
            rust::Statement::Semi(method(
                local("out"),
                "extend_from_slice",
                vec![rust::Expr::Ref {
                    target: Box::new(method(local("w"), "finish", Vec::new())),
                    mutable: false,
                }],
            )),
            rust::Statement::Tail(call(&["Ok"], vec![rust::Expr::Path(Path::single("()"))])),
        ],
    );

    // decode_xcdr1
    let decode_xcdr1 = function(
        "decode_xcdr1",
        vec![bytes_param("bytes")],
        decode_result(),
        vec![
            let_(
                "r",
                call(
                    &["zerodds_wire", "Reader", "with_max_align"],
                    vec![local("bytes"), little(), lit(8)],
                ),
                true,
            ),
            rust::Statement::Tail(call(
                &["Ok"],
                vec![rust::Expr::Try(Box::new(call(
                    &["Self", "decode_xcdr1"],
                    vec![mut_ref("r")],
                )))],
            )),
        ],
    );

    // encode_continuing
    let encode_continuing = function(
        "encode_continuing",
        vec![
            self_param,
            rust::Param {
                name: Some(Ident::new("prefix")),
                type_: vec_u8_mut,
                receiver: None,
            },
            bool_param("xcdr1"),
        ],
        encode_result(),
        vec![
            let_(
                "w",
                call(
                    &["zerodds_wire", "Writer", "continuing"],
                    vec![
                        little(),
                        cap(),
                        call(&["core", "mem", "take"], vec![local("prefix")]),
                    ],
                ),
                true,
            ),
            rust::Statement::Semi(rust::Expr::Try(Box::new(by_version(
                "encode_xcdr1",
                "encode",
                vec![local("self"), mut_ref("w")],
            )))),
            rust::Statement::Semi(rust::Expr::Assign {
                target: Box::new(rust::Expr::Deref(Box::new(local("prefix")))),
                value: Box::new(method(local("w"), "finish", Vec::new())),
            }),
            rust::Statement::Tail(call(&["Ok"], vec![rust::Expr::Path(Path::single("()"))])),
        ],
    );

    // decode_continuing
    let decode_continuing = function(
        "decode_continuing",
        vec![
            bytes_param("bytes"),
            rust::Param {
                name: Some(Ident::new("offset")),
                type_: rust::Type::named(Path::single("usize")),
                receiver: None,
            },
            bool_param("xcdr1"),
        ],
        decode_result(),
        vec![
            let_(
                "r",
                call(
                    &["zerodds_wire", "Reader", "with_max_align"],
                    vec![local("bytes"), little(), cap()],
                ),
                true,
            ),
            rust::Statement::Semi(rust::Expr::Try(Box::new(method(
                local("r"),
                "skip_to",
                vec![local("offset")],
            )))),
            rust::Statement::Tail(call(
                &["Ok"],
                vec![rust::Expr::Try(Box::new(by_version(
                    "decode_xcdr1",
                    "decode",
                    vec![mut_ref("r")],
                )))],
            )),
        ],
    );

    vec![
        encode_xcdr1,
        decode_xcdr1,
        encode_continuing,
        decode_continuing,
    ]
}

// == the error type ========================================================

fn error_enum(
    ast: &Ast,
    module: Id<ModuleNode>,
    service: &Service,
    exceptions: &[Raise],
) -> Result<rust::Enum, RustInterfaceError> {
    let mut variants = Vec::new();
    for raise in exceptions {
        variants.push(rust::Variant {
            doc: Doc::line(&format!(
                "The declared `{}` exception. DDS-RPC 1.0 §7.5.1.1.5.",
                raise.name
            )),
            attrs: Vec::new(),
            name: Ident::new(&rust::escape_keyword(&raise.name)),
            body: rust::VariantBody::Tuple(vec![exception_type(ast, module, raise.id)?]),
            discriminant: None,
        });
    }
    variants.push(rust::Variant {
        doc: Doc::lines(&[
            "A DDS-RPC 1.0 §7.11.1.3 remote/system exception, or a transport",
            "failure `zerodds_rpc::requester::Requester` reports — a timeout,",
            "an encode/decode failure, or a DCPS error.",
        ]),
        attrs: Vec::new(),
        name: Ident::new("Remote"),
        body: rust::VariantBody::Tuple(vec![rust::Type::named(Path::new(&[
            "zerodds_rpc",
            "error",
            "RpcError",
        ]))]),
        discriminant: None,
    });
    Ok(rust::Enum {
        doc: Doc::line(&format!("Every way a call to `{}` can fail.", service.name)),
        attrs: Vec::new(),
        derives: data_derives(),
        visibility: Visibility::Public,
        name: Ident::new(&error_type_name(service)),
        generics: Vec::new(),
        variants,
    })
}

// == the operation surface: trait, client, dispatcher =====================

/// One of the three things a `Result<T, {svc}Error>` value a client
/// method or a service trait method carries besides its error: the
/// operation's own return value, or one pure `out` parameter (never
/// `inout`, which travels through a `&mut` argument instead and is
/// never part of the `Ok` value).
#[derive(Clone, PartialEq, Eq)]
enum OutFieldSource {
    Return,
    Out(String),
    InOut(String),
}

fn same_source(a: &OutFieldSource, b: &OutFieldSource) -> bool {
    matches!((a, b), (OutFieldSource::Return, OutFieldSource::Return))
        || matches!((a, b), (OutFieldSource::Out(x), OutFieldSource::Out(y)) if x == y)
}

/// The `${I}_${op}_Out` structure's own fields, each paired with what
/// it is. The same order [`out_fields`] declares the structure in.
fn out_field_map(op: &Operation) -> Vec<(String, OutFieldSource)> {
    let mut out = Vec::new();
    for p in op.outputs() {
        let field_name = rust::escape_keyword(&p.name);
        let source = match p.direction {
            ParamDirection::InOut => OutFieldSource::InOut(p.name.clone()),
            _ => OutFieldSource::Out(p.name.clone()),
        };
        out.push((field_name, source));
    }
    if op.returns.is_some() {
        let taken: Vec<&str> = out.iter().map(|(name, _)| name.as_str()).collect();
        out.push((return_member_name(&taken), OutFieldSource::Return));
    }
    out
}

/// The positions the method's own `Ok` value carries: the return first,
/// when the operation has one, then every pure `out` parameter in
/// declaration order. Index 0 is tuple index `.0`, and so on — the same
/// order the client and the dispatcher both read, so the two sides
/// agree on which slot is which without a third reading of the
/// operation.
fn ok_positions(op: &Operation) -> Vec<OutFieldSource> {
    let mut out = Vec::new();
    if op.returns.is_some() {
        out.push(OutFieldSource::Return);
    }
    for p in op
        .params
        .iter()
        .filter(|p| matches!(p.direction, ParamDirection::Out))
    {
        out.push(OutFieldSource::Out(p.name.clone()));
    }
    out
}

/// Where `target` sits in the `Ok` value [`ok_positions`] lays out.
///
/// Both lists come from the same operation, so a miss is this module
/// contradicting itself — and answering position 0 for it would bind a
/// generated field to another field's value without a word.
fn position_index(
    positions: &[OutFieldSource],
    target: &OutFieldSource,
    op: &Operation,
) -> Result<usize, RustInterfaceError> {
    positions
        .iter()
        .position(|p| same_source(p, target))
        .ok_or_else(|| {
            RustInterfaceError::Interface(InterfaceError::Unresolved {
                construct: "out value's position in the Ok value",
                at: op.name.clone(),
            })
        })
}

/// The expression that reads `source` out of `binding`, a value shaped
/// by [`ok_type`]/[`ok_positions`] for this operation.
fn ok_component_expr(
    positions: &[OutFieldSource],
    source: &OutFieldSource,
    binding: &str,
    op: &Operation,
) -> Result<rust::Expr, RustInterfaceError> {
    if positions.len() <= 1 {
        Ok(rust::Expr::Path(Path::single(binding)))
    } else {
        let index = position_index(positions, source, op)?;
        Ok(field_expr(binding, &index.to_string()))
    }
}

fn ok_type(
    ast: &Ast,
    module: Id<ModuleNode>,
    op: &Operation,
) -> Result<rust::Type, RustInterfaceError> {
    let mut parts = Vec::new();
    if let Some(target) = op.returns {
        parts.push(rust_type_of(ast, module, Some(target))?);
    }
    for p in op
        .params
        .iter()
        .filter(|p| matches!(p.direction, ParamDirection::Out))
    {
        parts.push(rust_type_of(ast, module, Some(p.type_))?);
    }
    Ok(match parts.len() {
        0 => rust::Type::unit(),
        1 => parts.remove(0),
        _ => rust::Type::Tuple(parts),
    })
}

fn op_return_type(
    ast: &Ast,
    module: Id<ModuleNode>,
    service: &Service,
    op: &Operation,
) -> Result<rust::Type, RustInterfaceError> {
    Ok(rust::Type::generic(
        Path::single("Result"),
        vec![
            ok_type(ast, module, op)?,
            rust::Type::named(Path::single(&error_type_name(service))),
        ],
    ))
}

/// The parameters a service trait method, and a client method, take.
///
/// `in` by value, `inout` as `&mut T` — it has an initial value to
/// read — and a pure `out` is not a parameter at all: it has none to
/// read, and travels back through the `Ok` value instead
/// ([`ok_type`]), which is what lets a Rust caller call the method
/// without first inventing one.
fn op_params(
    ast: &Ast,
    module: Id<ModuleNode>,
    op: &Operation,
) -> Result<Vec<rust::Param>, RustInterfaceError> {
    let mut params = vec![rust::Param {
        name: None,
        type_: rust::Type::unit(),
        receiver: Some(rust::Receiver::Ref),
    }];
    for p in &op.params {
        let base = rust_type_of(ast, module, Some(p.type_))?;
        match p.direction {
            ParamDirection::In => params.push(rust::Param {
                name: Some(Ident::new(&rust::escape_keyword(&p.name))),
                type_: base,
                receiver: None,
            }),
            ParamDirection::InOut => params.push(rust::Param {
                name: Some(Ident::new(&rust::escape_keyword(&p.name))),
                type_: rust::Type::Ref {
                    target: Box::new(base),
                    mutable: true,
                    lifetime: None,
                },
                receiver: None,
            }),
            ParamDirection::Out => {}
        }
    }
    Ok(params)
}

/// A one-line doc for a generated operation: its `raises` clause where it
/// has one (DDS-RPC 1.0 §7.11.1.1.2 rule 6.c keeps the clause out of the
/// signature itself, so the doc line is where a reader finds it), else a
/// plain statement of what the IDL declared — every `pub` item this module
/// emits carries a doc, per the workspace's `missing_docs` lint.
fn method_doc(op: &Operation) -> String {
    op.raises_line().unwrap_or_else(|| {
        format!(
            "Generated from the IDL operation `{}`; raises nothing.",
            op.name
        )
    })
}

fn trait_method_signature(
    ast: &Ast,
    service: &Service,
    op: &Operation,
) -> Result<rust::Function, RustInterfaceError> {
    Ok(rust::Function {
        doc: Doc::line(&method_doc(op)),
        attrs: Vec::new(),
        visibility: Visibility::Private,
        name: Ident::new(&rust::escape_keyword(&op.name)),
        generics: Vec::new(),
        params: op_params(ast, service.module, op)?,
        returns: Some(op_return_type(ast, service.module, service, op)?),
        body: None,
    })
}

fn base_interface_path(
    ast: &Ast,
    from: Id<ModuleNode>,
    base: &interface::Base,
    suffix: &str,
) -> Result<Path, RustInterfaceError> {
    let node = ast.interface(base.id).map_err(|_| InterfaceError::NoNode {
        kind: "base interface",
        at: base.name.clone(),
    })?;
    relative_module_path(ast, from, node.module, &format!("{}{suffix}", node.name))
}

fn service_trait(ast: &Ast, service: &Service) -> Result<rust::Trait, RustInterfaceError> {
    let mut supertraits = Vec::new();
    for base in &service.bases {
        supertraits.push(rust::Type::named(base_interface_path(
            ast,
            service.module,
            base,
            "",
        )?));
    }
    let mut items = Vec::new();
    for op in service.all_operations() {
        items.push(rust::Item::Function(trait_method_signature(
            ast, service, &op,
        )?));
    }
    let mut doc = Doc::line(&format!(
        "The `{}` service. DDS-RPC 1.0 §7.5.1.1.3, §7.11.",
        service.name
    ));
    doc.push(
        "`out`/`inout` parameters are `&mut` arguments; a pure `out` parameter \
         besides the declared return travels back through the `Ok` value instead \
         — `()` for none, the bare value for one, a tuple (return first, then \
         every `out` in declaration order) for more than one.",
    );
    Ok(rust::Trait {
        doc,
        attrs: Vec::new(),
        visibility: Visibility::Public,
        name: Ident::new(&rust::escape_keyword(&service.name)),
        generics: Vec::new(),
        supertraits,
        items,
    })
}

// -- the client -------------------------------------------------------------

fn client_type_name(service: &Service) -> String {
    format!("{}Client", service.name)
}

fn client_field_name(base: &interface::Base) -> String {
    format!("base_{}", base.name.to_lowercase())
}

fn client_struct(ast: &Ast, service: &Service) -> Result<rust::Struct, RustInterfaceError> {
    let mut fields = vec![field_vis(
        "requester",
        rust::Type::generic(
            Path::new(&["zerodds_rpc", "requester", "Requester"]),
            vec![
                rust::Type::named(Path::single(&interface::call_union_name(&service.name))),
                rust::Type::named(Path::single(&interface::return_union_name(&service.name))),
            ],
        ),
        Visibility::Private,
        "The `Requester` this interface's own operations send `${I}_Call`s \
         through and read `${I}_Return`s back from — DDS-RPC 1.0 §7.9.",
    )];
    for base in &service.bases {
        let path = base_interface_path(ast, service.module, base, "Client")?;
        fields.push(field_vis(
            &client_field_name(base),
            rust::Type::named(path),
            Visibility::Public,
            &format!(
                "The `{}` client this interface's own base composes — DDS-RPC 1.0 \
                 §7.5.1.1.8 keeps its topic separate from this one's.",
                base.name
            ),
        ));
    }
    let mut doc = Doc::line("A typed DDS-RPC client. DDS-RPC 1.0 §7.11.1.5.4/§7.9.");
    doc.push(
        "A base interface's operations are not repeated here — DDS-RPC 1.0 keeps a \
         base's topic separate from a derived interface's own (§7.5.1.1.8) — they \
         are reachable through the base client field this struct composes, one per \
         parent interface, so a call to an inherited operation is one field access \
         away rather than absent.",
    );
    Ok(rust::Struct {
        body_text: zerodds_lang_cst::common::BodyText::none(),
        doc,
        attrs: Vec::new(),
        derives: Vec::new(),
        visibility: Visibility::Public,
        name: Ident::new(&client_type_name(service)),
        generics: Vec::new(),
        body: rust::StructBody::Named(fields),
    })
}

fn dcps_participant_type() -> rust::Type {
    rust::Type::Ref {
        target: Box::new(rust::Type::named(Path::new(&[
            "zerodds_dcps",
            "participant",
            "DomainParticipant",
        ]))),
        mutable: false,
        lifetime: None,
    }
}

fn rpc_qos_type() -> rust::Type {
    rust::Type::Ref {
        target: Box::new(rust::Type::named(Path::new(&[
            "zerodds_rpc",
            "qos_profile",
            "RpcQos",
        ]))),
        mutable: false,
        lifetime: None,
    }
}

fn rpc_result_of(ok: rust::Type) -> rust::Type {
    rust::Type::generic(Path::new(&["zerodds_rpc", "error", "RpcResult"]), vec![ok])
}

fn try_call(callee: Path, args: Vec<rust::Expr>) -> rust::Expr {
    rust::Expr::Try(Box::new(rust::Expr::Call {
        callee: Box::new(rust::Expr::Path(callee)),
        args,
    }))
}

/// `"text".to_string()` — an owned `String` from a literal, for a field
/// whose type is `String`/`Option<String>` rather than `&str`.
fn owned_string_literal(text: &str) -> rust::Expr {
    rust::Expr::MethodCall {
        receiver: Box::new(rust::Expr::Literal(Literal::string(text))),
        method: Ident::new("to_string"),
        args: Vec::new(),
    }
}

/// `Some("text".to_string())` / `None::<String>` for an `Option<String>`
/// field, from an `Option<String>` already read off the model.
fn optional_owned_string(value: Option<&str>) -> rust::Expr {
    match value {
        Some(text) => rust::Expr::Call {
            callee: Box::new(rust::Expr::Path(Path::single("Some"))),
            args: vec![owned_string_literal(text)],
        },
        None => rust::Expr::Path(Path::new(&["None"])),
    }
}

/// `&zerodds_rpc::topic_naming::AnnotatedTopics { .. }` for `service` —
/// its own `@service`/`@DDSRequestTopic`/`@DDSReplyTopic` values
/// (`adapter::interface::Service::service_name`/`request_topic`/
/// `reply_topic`, already read off the IDL's facets), no run-time
/// override (`TopicNameOverrides::default()` — a caller with one does
/// not go through the generated `new`/`serve` at all). DDS-RPC 1.0
/// §7.9/§7.10 give the `Requester`/`Replier` exactly these three
/// service-declared inputs; this is the generated binding reading them
/// instead of leaving every service at the `Calculator_Service_Request`
/// default regardless of what its own IDL declared.
fn annotated_topics_expr(service: &Service) -> rust::Expr {
    rust::Expr::Ref {
        target: Box::new(rust::Expr::StructLit {
            path: Path::new(&["zerodds_rpc", "topic_naming", "AnnotatedTopics"]),
            fields: vec![
                (
                    Ident::new("service_name"),
                    optional_owned_string(Some(service.service_name.as_str())),
                ),
                (
                    Ident::new("request_topic"),
                    optional_owned_string(service.request_topic.as_deref()),
                ),
                (
                    Ident::new("reply_topic"),
                    optional_owned_string(service.reply_topic.as_deref()),
                ),
                (
                    Ident::new("overrides"),
                    rust::Expr::Call {
                        callee: Box::new(rust::Expr::Path(Path::new(&[
                            "zerodds_rpc",
                            "topic_naming",
                            "TopicNameOverrides",
                            "none",
                        ]))),
                        args: Vec::new(),
                    },
                ),
            ],
            base: None,
        }),
        mutable: false,
    }
}

/// `&zerodds_rpc::endpoint_options::RpcEndpointOptions::for_interface(scope,
/// name).with_parent(..)` for `service`: the DDS type names of its topics
/// — `${interfaceName}_Request` and `_Reply` in the module the interface
/// is declared in (§7.5.1.1.6, §7.5.1.1.7) — and the base interfaces whose
/// topics are its aliases (§7.4.3).
fn endpoint_options_expr(ast: &Ast, service: &Service) -> Result<rust::Expr, RustInterfaceError> {
    Ok(rust::Expr::Ref {
        target: Box::new(endpoint_options_value(ast, service)?),
        mutable: false,
    })
}

/// [`endpoint_options_expr`] without the reference: the options by value.
fn endpoint_options_value(ast: &Ast, service: &Service) -> Result<rust::Expr, RustInterfaceError> {
    let scope = interface::module_chain(ast, service.module)?
        .into_iter()
        .map(|(_, name)| name)
        .collect::<Vec<_>>()
        .join("::");
    let mut options = rust::Expr::Call {
        callee: Box::new(rust::Expr::Path(Path::new(&[
            "zerodds_rpc",
            "endpoint_options",
            "RpcEndpointOptions",
            "for_interface",
        ]))),
        args: vec![
            rust::Expr::Literal(Literal::string(&scope)),
            rust::Expr::Literal(Literal::string(&service.name)),
        ],
    };
    for base in &service.bases {
        options = rust::Expr::MethodCall {
            receiver: Box::new(options),
            method: Ident::new("with_parent"),
            args: vec![rust::Expr::Literal(Literal::string(&base.qualified_name))],
        };
    }
    if service.mapping.is_enhanced() {
        options = rust::Expr::MethodCall {
            receiver: Box::new(options),
            method: Ident::new("with_mapping"),
            args: vec![rust::Expr::Path(Path::new(&[
                "zerodds_rpc",
                "endpoint_options",
                "RpcMapping",
                "Enhanced",
            ]))],
        };
        // The XTypes description of the two topic types, when every type
        // they name has an object. A recursive type, or one XTypes 1.3 §7.3
        // has no identifier for, leaves the topics matched by type name.
        let model = interface::enhanced_service(ast, service, service.mapping.empty_style())?;
        if let Ok(objects) = type_information::service_type_objects(ast, service, &model) {
            options = rust::Expr::MethodCall {
                receiver: Box::new(options),
                method: Ident::new("with_type_information"),
                args: vec![type_information_expr(&objects)],
            };
        }
    }
    Ok(options)
}

/// `zerodds_rpc::endpoint_options::RpcTypeInformation { request, reply }`
/// over the bytes of `objects`.
fn type_information_expr(objects: &type_information::ServiceTypeObjects) -> rust::Expr {
    let topic = |topic: &type_information::TopicTypeObjects| rust::Expr::StructLit {
        path: Path::new(&["zerodds_rpc", "endpoint_options", "TopicTypeBytes"]),
        fields: vec![
            (
                Ident::new("object"),
                rust::Expr::Literal(Literal::Bytes(topic.object.clone())),
            ),
            (
                Ident::new("dependencies"),
                rust::Expr::Ref {
                    target: Box::new(rust::Expr::Array(
                        topic
                            .dependencies
                            .iter()
                            .map(|bytes| rust::Expr::Literal(Literal::Bytes(bytes.clone())))
                            .collect(),
                    )),
                    mutable: false,
                },
            ),
        ],
        base: None,
    };
    rust::Expr::StructLit {
        path: Path::new(&["zerodds_rpc", "endpoint_options", "RpcTypeInformation"]),
        fields: vec![
            (Ident::new("request"), topic(&objects.request)),
            (Ident::new("reply"), topic(&objects.reply)),
        ],
        base: None,
    }
}

fn binding_style_expr() -> rust::Expr {
    rust::Expr::Path(Path::new(&[
        "zerodds_rpc",
        "topic_naming",
        "BindingStyle",
        "FunctionCall",
    ]))
}

fn client_impl(ast: &Ast, service: &Service) -> Result<rust::Item, RustInterfaceError> {
    let mut new_fields = vec![(
        Ident::new("requester"),
        try_call(
            Path::new(&["zerodds_rpc", "requester", "Requester", "with_options"]),
            vec![
                rust::Expr::Path(Path::single("participant")),
                rust::Expr::Literal(Literal::string(&service.qualified_name)),
                rust::Expr::Literal(Literal::string("")),
                rust::Expr::Path(Path::single("qos")),
                binding_style_expr(),
                annotated_topics_expr(service),
                endpoint_options_expr(ast, service)?,
            ],
        ),
    )];
    for base in &service.bases {
        let path = base_interface_path(ast, service.module, base, "Client")?;
        new_fields.push((
            Ident::new(&client_field_name(base)),
            try_call(
                path.join("new"),
                vec![
                    rust::Expr::Path(Path::single("participant")),
                    rust::Expr::Path(Path::single("qos")),
                ],
            ),
        ));
    }
    let new_fn = rust::Item::Function(rust::Function {
        doc: Doc::line(
            "A client bound to a `DomainParticipant`, one `Requester` per interface level.",
        ),
        attrs: Vec::new(),
        visibility: Visibility::Public,
        name: Ident::new("new"),
        generics: Vec::new(),
        params: vec![
            rust::Param {
                name: Some(Ident::new("participant")),
                type_: dcps_participant_type(),
                receiver: None,
            },
            rust::Param {
                name: Some(Ident::new("qos")),
                type_: rpc_qos_type(),
                receiver: None,
            },
        ],
        returns: Some(rpc_result_of(rust::Type::named(Path::single("Self")))),
        body: Some(rust::Block {
            statements: vec![rust::Statement::Tail(rust::Expr::Call {
                callee: Box::new(rust::Expr::Path(Path::single("Ok"))),
                args: vec![rust::Expr::StructLit {
                    path: Path::single("Self"),
                    fields: new_fields,
                    base: None,
                }],
            })],
        }),
    });

    let options_fn = rust::Item::Function(rust::Function {
        doc: Doc::line(
            "The options this interface's endpoints are created with: the DDS type names of \
             the topics, the base interfaces, the service mapping and, under the Enhanced \
             mapping, the TypeObjects of the topic types. An endpoint written by hand for \
             the same topics, on a participant that also holds this binding, has to use \
             them: a topic name is reused only with the same type metadata.",
        ),
        attrs: Vec::new(),
        visibility: Visibility::Public,
        name: Ident::new("endpoint_options"),
        generics: Vec::new(),
        params: Vec::new(),
        returns: Some(rust::Type::named(Path::new(&[
            "zerodds_rpc",
            "endpoint_options",
            "RpcEndpointOptions",
        ]))),
        body: Some(rust::Block {
            statements: vec![rust::Statement::Tail(endpoint_options_value(ast, service)?)],
        }),
    });
    let mut items = vec![new_fn, options_fn];
    if let Some(request_profile) = service.qos_request_profile.as_deref() {
        items.push(declared_qos_fn(
            "new_with_declared_qos",
            "A client whose QoS is the one `@Qos` declares for this interface (DDS-RPC 1.0 §7.10.1).",
            request_profile,
            service.qos_reply_profile.as_deref(),
            None,
            rpc_result_of(rust::Type::named(Path::single("Self"))),
            &["Self", "new"],
        ));
    }
    for op in service.all_operations() {
        items.push(rust::Item::Function(client_method(ast, service, &op)?));
    }

    Ok(rust::Item::Impl(rust::Impl {
        doc: Doc::none(),
        attrs: Vec::new(),
        generics: Vec::new(),
        trait_: None,
        self_type: rust::Type::named(Path::single(&client_type_name(service))),
        items,
    }))
}

/// A constructor whose QoS is `@Qos(RequestProfile, ReplyProfile)`
/// (§7.10.1): reads the profiles from their URLs and calls `delegate`
/// with the result. `receiver` is the `self` the delegate also takes.
fn declared_qos_fn(
    name: &str,
    doc: &str,
    request_profile: &str,
    reply_profile: Option<&str>,
    receiver: Option<rust::Receiver>,
    returns: rust::Type,
    delegate: &[&str],
) -> rust::Item {
    let reply = match reply_profile {
        Some(url) => rust::Expr::Call {
            callee: Box::new(rust::Expr::Path(Path::single("Some"))),
            args: vec![rust::Expr::Literal(Literal::string(url))],
        },
        None => rust::Expr::Path(Path::single("None")),
    };
    let load_qos = rust::Statement::Let {
        name: Ident::new("qos"),
        type_: None,
        value: Some(try_call(
            Path::new(&[
                "zerodds_rpc",
                "qos_profile",
                "RpcQos",
                "from_service_profiles",
            ]),
            vec![rust::Expr::Literal(Literal::string(request_profile)), reply],
        )),
        mutable: false,
    };
    let mut call_args = Vec::new();
    if receiver.is_some() {
        call_args.push(rust::Expr::Path(Path::single("self")));
    }
    call_args.push(rust::Expr::Path(Path::single("participant")));
    call_args.push(rust::Expr::Ref {
        target: Box::new(rust::Expr::Path(Path::single("qos"))),
        mutable: false,
    });
    let mut params = Vec::new();
    if let Some(receiver) = receiver {
        params.push(rust::Param {
            name: None,
            type_: rust::Type::unit(),
            receiver: Some(receiver),
        });
    }
    params.push(rust::Param {
        name: Some(Ident::new("participant")),
        type_: dcps_participant_type(),
        receiver: None,
    });
    rust::Item::Function(rust::Function {
        doc: Doc::line(doc),
        attrs: Vec::new(),
        visibility: Visibility::Public,
        name: Ident::new(name),
        generics: Vec::new(),
        params,
        returns: Some(returns),
        body: Some(rust::Block {
            statements: vec![
                load_qos,
                rust::Statement::Tail(rust::Expr::Call {
                    callee: Box::new(rust::Expr::Path(Path::new(delegate))),
                    args: call_args,
                }),
            ],
        }),
    })
}

fn requester_field() -> rust::Expr {
    field_expr("self", "requester")
}

fn map_err_remote(service: &Service, value: rust::Expr) -> rust::Expr {
    rust::Expr::MethodCall {
        receiver: Box::new(value),
        method: Ident::new("map_err"),
        args: vec![rust::Expr::Path(Path::new(&[
            &error_type_name(service),
            "Remote",
        ]))],
    }
}

/// Whether `rustc` derives `Copy` for a [`rust::Type`] this generator
/// materialises — the scalar primitives DDS-RPC 1.0's own IDL primitive
/// mapping produces. Anything else (`String`, a generated `struct`, a
/// generated `enum`) is `Clone`-only.
fn is_copy_primitive(type_: &rust::Type) -> bool {
    matches!(
        type_,
        rust::Type::Primitive(
            rust::Primitive::Bool
                | rust::Primitive::Char
                | rust::Primitive::I8
                | rust::Primitive::I16
                | rust::Primitive::I32
                | rust::Primitive::I64
                | rust::Primitive::I128
                | rust::Primitive::U8
                | rust::Primitive::U16
                | rust::Primitive::U32
                | rust::Primitive::U64
                | rust::Primitive::U128
                | rust::Primitive::F32
                | rust::Primitive::F64
        )
    )
}

fn in_struct_literal(
    ast: &Ast,
    module: Id<ModuleNode>,
    op: &Operation,
    path: Path,
    empty: interface::EmptyOperationStyle,
) -> Result<rust::Expr, RustInterfaceError> {
    let mut fields = Vec::new();
    for p in op.inputs() {
        let name = rust::escape_keyword(&p.name);
        // An `in` parameter (`op_params`) is already owned by this method,
        // so it moves straight into the payload. An `inout` one is
        // `&mut T` (`rust::Type::Ref { mutable: true, .. }`): the payload
        // needs an owned `T` and the caller keeps the reference to assign
        // the reply back into (`client_reply_match`), so it is read
        // through the reference — by value (`*x`) where `T` is one of
        // DDS-RPC 1.0's `Copy` primitives, else by `.clone()`, so neither
        // `clippy::clone_on_copy` nor a move out of a `&mut` reference.
        let value = if matches!(p.direction, ParamDirection::InOut) {
            let target = rust_type_of(ast, module, Some(p.type_))?;
            if is_copy_primitive(&target) {
                rust::Expr::Deref(Box::new(rust::Expr::Path(Path::single(&name))))
            } else {
                rust::Expr::MethodCall {
                    receiver: Box::new(rust::Expr::Path(Path::single(&name))),
                    method: Ident::new("clone"),
                    args: Vec::new(),
                }
            }
        } else {
            rust::Expr::Path(Path::single(&name))
        };
        fields.push((Ident::new(&name), value));
    }
    if fields.is_empty() && empty == interface::EmptyOperationStyle::DummyMember {
        fields.push((
            Ident::new(interface::DUMMY_MEMBER),
            rust::Expr::Literal(Literal::int(0)),
        ));
    }
    Ok(rust::Expr::StructLit {
        path,
        fields,
        base: None,
    })
}

/// A client method's body, for one operation of `service`.
///
/// Send, block for the reply, and take the reply union's own
/// case apart into the operation's `Ok`/`Err`. A reply whose case does
/// not match the operation called is a protocol error and not
/// constructed by this binding's own dispatcher; it is reported through
/// `{svc}Error::Remote` rather than panicked on.
fn client_method(
    ast: &Ast,
    service: &Service,
    op: &Operation,
) -> Result<rust::Function, RustInterfaceError> {
    let module = service.module;
    let params = op_params(ast, module, op)?;
    let returns = op_return_type(ast, module, service, op)?;
    let variant = variant_name(&op.name);
    let in_struct_path = Path::single(&interface::in_struct_name(&service.name, &op.name));
    let call_path = Path::new(&[&interface::call_union_name(&service.name), &variant]);

    let build_payload = rust::Statement::Let {
        name: Ident::new("request_payload"),
        type_: None,
        value: Some(in_struct_literal(
            ast,
            module,
            op,
            in_struct_path,
            service.mapping.empty_style(),
        )?),
        mutable: false,
    };
    let build_call = rust::Statement::Let {
        name: Ident::new("call"),
        type_: None,
        value: Some(rust::Expr::Call {
            callee: Box::new(rust::Expr::Path(call_path)),
            args: vec![rust::Expr::Path(Path::single("request_payload"))],
        }),
        mutable: false,
    };

    let send = rust::Expr::MethodCall {
        receiver: Box::new(requester_field()),
        method: Ident::new("send_request_blocking"),
        args: vec![
            rust::Expr::Ref {
                target: Box::new(rust::Expr::Path(Path::single("call"))),
                mutable: false,
            },
            rust::Expr::Path(Path::single("None")),
        ],
    };
    let reply_let = rust::Statement::Let {
        name: Ident::new("reply"),
        type_: None,
        value: Some(rust::Expr::Try(Box::new(map_err_remote(service, send)))),
        mutable: false,
    };
    let statements = vec![
        build_payload,
        build_call,
        reply_let,
        client_reply_match(service, op, &variant)?,
    ];

    Ok(rust::Function {
        doc: Doc::line(&method_doc(op)),
        attrs: Vec::new(),
        visibility: Visibility::Public,
        name: Ident::new(&rust::escape_keyword(&op.name)),
        generics: Vec::new(),
        params,
        returns: Some(returns),
        body: Some(rust::Block { statements }),
    })
}

fn client_reply_match(
    service: &Service,
    op: &Operation,
    variant: &str,
) -> Result<rust::Statement, RustInterfaceError> {
    let map = out_field_map(op);
    let positions = ok_positions(op);
    let mut assigns = Vec::new();
    let mut ok_values = Vec::new();
    for (field_name, source) in &map {
        match source {
            OutFieldSource::InOut(param_name) => {
                assigns.push(rust::Statement::Semi(rust::Expr::Assign {
                    target: Box::new(rust::Expr::Deref(Box::new(rust::Expr::Path(Path::single(
                        &rust::escape_keyword(param_name),
                    ))))),
                    value: Box::new(field_expr("out_val", field_name)),
                }));
            }
            OutFieldSource::Return | OutFieldSource::Out(_) => {
                let index = position_index(&positions, source, op)?;
                ok_values.push((index, field_expr("out_val", field_name)));
            }
        }
    }
    ok_values.sort_by_key(|(index, _)| *index);
    let ok_expr = match ok_values.len() {
        0 => ok_unit(),
        1 => rust::Expr::Call {
            callee: Box::new(rust::Expr::Path(Path::single("Ok"))),
            args: vec![ok_values.remove(0).1],
        },
        _ => rust::Expr::Call {
            callee: Box::new(rust::Expr::Path(Path::single("Ok"))),
            args: vec![rust::Expr::Tuple(
                ok_values.into_iter().map(|(_, expr)| expr).collect(),
            )],
        },
    };
    let mut ok_arm_statements = assigns;
    ok_arm_statements.push(rust::Statement::Tail(ok_expr));

    let result_name = interface::result_union_name(&service.name, &op.name);
    // `out_val` is read by `assigns`/`ok_values` above only where the
    // reply carries an `out`, `inout`, or return value (DDS-RPC 1.0
    // §7.5.1.1.5); an operation whose `${I}_${op}_Out` is empty (no
    // outputs, `void` return) binds a wildcard instead so `rustc` does
    // not warn on the unread payload.
    let ok_binding = if map.is_empty() {
        rust::Pattern::Wildcard
    } else {
        rust::Pattern::Binding(Ident::new("out_val"))
    };
    let mut result_arms = vec![rust::MatchArm {
        pattern: rust::Pattern::TupleStruct {
            path: Path::new(&[&result_name, "Ok"]),
            elements: vec![ok_binding],
        },
        guard: None,
        body: rust::Expr::Block(rust::Block {
            statements: ok_arm_statements,
        }),
    }];
    for raise in &op.raises {
        let raise_variant = rust::escape_keyword(&raise.name);
        result_arms.push(rust::MatchArm {
            pattern: rust::Pattern::TupleStruct {
                path: Path::new(&[&result_name, &raise_variant]),
                elements: vec![rust::Pattern::Binding(Ident::new("exc"))],
            },
            guard: None,
            body: rust::Expr::Call {
                callee: Box::new(rust::Expr::Path(Path::single("Err"))),
                args: vec![rust::Expr::Call {
                    callee: Box::new(rust::Expr::Path(Path::new(&[
                        &error_type_name(service),
                        &raise_variant,
                    ]))),
                    args: vec![rust::Expr::Path(Path::single("exc"))],
                }],
            },
        });
    }

    // §7.7.2.5.6: an Enhanced `Result` that carries an exception member
    // this version of the interface does not declare is
    // `REMOTE_EX_UNKNOWN_EXCEPTION` for the client.
    if service.mapping.is_enhanced() {
        result_arms.push(rust::MatchArm {
            pattern: rust::Pattern::TupleStruct {
                path: Path::new(&[&result_name, UNKNOWN_EX_VARIANT]),
                elements: vec![rust::Pattern::Wildcard],
            },
            guard: None,
            body: rust::Expr::Call {
                callee: Box::new(rust::Expr::Path(Path::single("Err"))),
                args: vec![rust::Expr::Call {
                    callee: Box::new(rust::Expr::Path(Path::new(&[
                        &error_type_name(service),
                        "Remote",
                    ]))),
                    args: vec![rust::Expr::Call {
                        callee: Box::new(rust::Expr::Path(Path::new(&[
                            "zerodds_rpc",
                            "error",
                            "RpcError",
                            "RemoteException",
                        ]))),
                        args: vec![rust::Expr::MethodCall {
                            receiver: Box::new(rust::Expr::Path(remote_exception_code(
                                "UnknownException",
                            ))),
                            method: Ident::new("as_u32"),
                            args: Vec::new(),
                        }],
                    }],
                }],
            },
        });
    }

    let return_name = interface::return_union_name(&service.name);
    let outer_match = rust::Expr::Match {
        scrutinee: Box::new(rust::Expr::Path(Path::single("reply"))),
        arms: vec![
            rust::MatchArm {
                pattern: rust::Pattern::TupleStruct {
                    path: Path::new(&[&return_name, variant]),
                    elements: vec![rust::Pattern::Binding(Ident::new("result"))],
                },
                guard: None,
                body: rust::Expr::Match {
                    scrutinee: Box::new(rust::Expr::Path(Path::single("result"))),
                    arms: result_arms,
                },
            },
            rust::MatchArm {
                pattern: rust::Pattern::Wildcard,
                guard: None,
                body: rust::Expr::Call {
                    callee: Box::new(rust::Expr::Path(Path::single("Err"))),
                    args: vec![rust::Expr::Call {
                        callee: Box::new(rust::Expr::Path(Path::new(&[
                            &error_type_name(service),
                            "Remote",
                        ]))),
                        args: vec![rust::Expr::Call {
                            callee: Box::new(rust::Expr::Path(Path::new(&[
                                "zerodds_rpc",
                                "error",
                                "RpcError",
                                "Dcps",
                            ]))),
                            args: vec![rust::Expr::MethodCall {
                                receiver: Box::new(rust::Expr::Literal(Literal::string(&format!(
                                    "reply carried a case other than `{}`",
                                    op.name
                                )))),
                                method: Ident::new("to_string"),
                                args: Vec::new(),
                            }],
                        }],
                    }],
                },
            },
        ],
    };
    Ok(rust::Statement::Tail(outer_match))
}

// -- the dispatcher -----------------------------------------------------------

fn dispatcher_type_name(service: &Service) -> String {
    format!("{}Dispatcher", service.name)
}

fn remote_exception_code(variant: &str) -> Path {
    Path::new(&[
        "zerodds_rpc",
        "common_types",
        "RemoteExceptionCode",
        variant,
    ])
}

fn service_bound(service: &Service) -> Vec<rust::Type> {
    vec![
        rust::Type::named(Path::single(&rust::escape_keyword(&service.name))),
        rust::Type::named(Path::single("Send")),
        rust::Type::named(Path::single("Sync")),
        rust::Type::named(Path::single("'static")),
    ]
}

fn dispatcher_generic() -> rust::GenericParam {
    // Filled in per service by the caller — see `dispatcher_struct`.
    rust::GenericParam {
        name: Ident::new("S"),
        bounds: Vec::new(),
        lifetime: false,
    }
}

fn dispatcher_struct(service: &Service) -> rust::Struct {
    let mut generic = dispatcher_generic();
    generic.bounds = service_bound(service);
    let mut doc =
        Doc::line("Serves one service implementation over a `zerodds_rpc::replier::Replier`.");
    doc.push(
        "A base interface's operations are dispatched by that base's own \
         `Dispatcher` and `Replier`, run alongside this one — DDS-RPC 1.0 keeps \
         a base's topic separate from a derived interface's own (§7.5.1.1.8), \
         and so does this binding.",
    );
    rust::Struct {
        body_text: zerodds_lang_cst::common::BodyText::none(),
        doc,
        attrs: Vec::new(),
        derives: Vec::new(),
        visibility: Visibility::Public,
        name: Ident::new(&dispatcher_type_name(service)),
        generics: vec![generic],
        body: rust::StructBody::Named(vec![field_vis(
            "service",
            rust::Type::named(Path::single("S")),
            Visibility::Private,
            "The implementation this dispatcher hands each `${I}_Call` to — \
             DDS-RPC 1.0 §7.10's Replier, over the `${I}` trait.",
        )]),
    }
}

fn dispatcher_out_literal(
    service: &Service,
    op: &Operation,
    qualify: &impl Fn(&str) -> Result<Path, RustInterfaceError>,
) -> Result<rust::Expr, RustInterfaceError> {
    let map = out_field_map(op);
    let out_path = qualify(&interface::out_struct_name(&service.name, &op.name))?;
    if map.is_empty() {
        let fields = if service.mapping.empty_style() == interface::EmptyOperationStyle::DummyMember
        {
            vec![(
                Ident::new(interface::DUMMY_MEMBER),
                rust::Expr::Literal(Literal::int(0)),
            )]
        } else {
            Vec::new()
        };
        return Ok(rust::Expr::StructLit {
            path: out_path,
            fields,
            base: None,
        });
    }
    let positions = ok_positions(op);
    let mut fields = Vec::new();
    for (field_name, source) in &map {
        let expr = match source {
            OutFieldSource::InOut(param_name) => {
                rust::Expr::Path(Path::single(&rust::escape_keyword(param_name)))
            }
            OutFieldSource::Return | OutFieldSource::Out(_) => {
                ok_component_expr(&positions, source, "value", op)?
            }
        };
        fields.push((Ident::new(field_name), expr));
    }
    Ok(rust::Expr::StructLit {
        path: out_path,
        fields,
        base: None,
    })
}

/// The dispatcher call arm's binding for the `${I}_${op}_In` payload: a
/// name where `dispatcher_call_args`/`dispatcher_inout_locals` read one of
/// its fields (any `in`/`inout` parameter), else a wildcard — an operation
/// with none, such as a no-argument `reset()`, binds nothing so `rustc`
/// does not warn on the unread name.
fn in_binding_pattern(op: &Operation) -> rust::Pattern {
    let reads_in = op
        .params
        .iter()
        .any(|p| matches!(p.direction, ParamDirection::In | ParamDirection::InOut));
    if reads_in {
        rust::Pattern::Binding(Ident::new("in_"))
    } else {
        rust::Pattern::Wildcard
    }
}

fn dispatcher_call_args(op: &Operation) -> Vec<rust::Expr> {
    let mut args = Vec::new();
    for p in &op.params {
        let name = rust::escape_keyword(&p.name);
        match p.direction {
            ParamDirection::In => args.push(field_expr("in_", &name)),
            ParamDirection::InOut => args.push(rust::Expr::Ref {
                target: Box::new(rust::Expr::Path(Path::single(&name))),
                mutable: true,
            }),
            ParamDirection::Out => {}
        }
    }
    args
}

fn dispatcher_inout_locals(op: &Operation) -> Vec<rust::Statement> {
    op.params
        .iter()
        .filter(|p| matches!(p.direction, ParamDirection::InOut))
        .map(|p| {
            let name = rust::escape_keyword(&p.name);
            rust::Statement::Let {
                name: Ident::new(&name),
                type_: None,
                value: Some(field_expr("in_", &name)),
                mutable: true,
            }
        })
        .collect()
}

/// One `${I}_Call` variant's arm in the dispatcher's `handle`.
fn dispatcher_call_arm(
    service: &Service,
    op: &Operation,
) -> Result<rust::MatchArm, RustInterfaceError> {
    dispatcher_call_arm_via(service, service, op, &|name| Ok(Path::single(name)))
}

/// The arm of `dispatching`'s dispatcher for `op`, which `declaring`
/// declares: the same interface for an operation of its own, a base for an
/// inherited one (DDS-RPC 1.0 §7.5.1.2.6: the Enhanced mapping's request
/// choice holds every inherited operation, and the derived service
/// implements the base trait). The topic types are `dispatching`'s; the
/// `Result`, `Out` and error types are `declaring`'s, reached through
/// `qualify`.
fn dispatcher_call_arm_via(
    dispatching: &Service,
    declaring: &Service,
    op: &Operation,
    qualify: &impl Fn(&str) -> Result<Path, RustInterfaceError>,
) -> Result<rust::MatchArm, RustInterfaceError> {
    let service = dispatching;
    let variant = variant_name(&op.name);
    let call_name = interface::call_union_name(&service.name);
    let return_name = interface::return_union_name(&service.name);

    let mut statements = dispatcher_inout_locals(op);
    let call_expr = rust::Expr::MethodCall {
        receiver: Box::new(field_expr("self", "service")),
        method: Ident::new(&rust::escape_keyword(&op.name)),
        args: dispatcher_call_args(op),
    };

    let ok_pattern = if ok_positions(op).is_empty() {
        rust::Pattern::Wildcard
    } else {
        rust::Pattern::Binding(Ident::new("value"))
    };
    let result_name = interface::result_union_name(&declaring.name, &op.name);
    let ok_arm = rust::MatchArm {
        pattern: rust::Pattern::TupleStruct {
            path: Path::single("Ok"),
            elements: vec![ok_pattern],
        },
        guard: None,
        body: rust::Expr::Call {
            callee: Box::new(rust::Expr::Path(Path::single("Ok"))),
            args: vec![rust::Expr::Call {
                callee: Box::new(rust::Expr::Path(Path::new(&[&return_name, &variant]))),
                args: vec![rust::Expr::Call {
                    callee: Box::new(rust::Expr::Path(qualify(&result_name)?.join("Ok"))),
                    args: vec![dispatcher_out_literal(declaring, op, qualify)?],
                }],
            }],
        },
    };
    let mut arms = vec![ok_arm];
    for raise in &op.raises {
        let raise_variant = rust::escape_keyword(&raise.name);
        arms.push(rust::MatchArm {
            pattern: rust::Pattern::TupleStruct {
                path: Path::single("Err"),
                elements: vec![rust::Pattern::TupleStruct {
                    path: qualify(&error_type_name(declaring))?.join(&raise_variant),
                    elements: vec![rust::Pattern::Binding(Ident::new("exc"))],
                }],
            },
            guard: None,
            body: rust::Expr::Call {
                callee: Box::new(rust::Expr::Path(Path::single("Ok"))),
                args: vec![rust::Expr::Call {
                    callee: Box::new(rust::Expr::Path(Path::new(&[&return_name, &variant]))),
                    args: vec![rust::Expr::Call {
                        callee: Box::new(rust::Expr::Path(
                            qualify(&result_name)?.join(&raise_variant),
                        )),
                        args: vec![rust::Expr::Path(Path::single("exc"))],
                    }],
                }],
            },
        });
    }
    // Exhaustive over `{svc}Error` whatever else it carries: a
    // `Remote(_)` this service should not have produced, or an
    // exception variant `op` does not declare because some *other*
    // operation of the same interface raises it — `{svc}Error` is
    // shared. Either is an exception the caller's declared set does not
    // name; §7.5.2 makes that `REMOTE_EX_UNKNOWN_EXCEPTION`.
    arms.push(rust::MatchArm {
        pattern: rust::Pattern::Wildcard,
        guard: None,
        body: rust::Expr::Call {
            callee: Box::new(rust::Expr::Path(Path::single("Err"))),
            args: vec![rust::Expr::Path(remote_exception_code("UnknownException"))],
        },
    });
    statements.push(rust::Statement::Tail(rust::Expr::Match {
        scrutinee: Box::new(call_expr),
        arms,
    }));

    Ok(rust::MatchArm {
        pattern: rust::Pattern::TupleStruct {
            path: Path::new(&[&call_name, &variant]),
            elements: vec![in_binding_pattern(op)],
        },
        guard: None,
        body: rust::Expr::Block(rust::Block { statements }),
    })
}

fn dispatcher_handle_fn(
    service: &Service,
    ops: &[Operation],
) -> Result<rust::Item, RustInterfaceError> {
    let call_name = interface::call_union_name(&service.name);
    let mut arms: Vec<rust::MatchArm> = ops
        .iter()
        .map(|op| dispatcher_call_arm(service, op))
        .collect::<Result<_, _>>()?;
    arms.push(rust::MatchArm {
        pattern: rust::Pattern::TupleStruct {
            path: Path::new(&[&call_name, UNKNOWN_OP_VARIANT]),
            elements: vec![rust::Pattern::Wildcard],
        },
        guard: None,
        body: rust::Expr::Call {
            callee: Box::new(rust::Expr::Path(Path::single("Err"))),
            args: vec![rust::Expr::Path(remote_exception_code("Unsupported"))],
        },
    });
    Ok(rust::Item::Function(rust::Function {
        doc: Doc::none(),
        attrs: Vec::new(),
        visibility: Visibility::Private,
        name: Ident::new("handle"),
        generics: Vec::new(),
        params: vec![
            rust::Param {
                name: None,
                type_: rust::Type::unit(),
                receiver: Some(rust::Receiver::Ref),
            },
            rust::Param {
                name: Some(Ident::new("request")),
                type_: rust::Type::named(Path::single(&call_name)),
                receiver: None,
            },
        ],
        returns: Some(rust::Type::generic(
            Path::single("Result"),
            vec![
                rust::Type::named(Path::single(&interface::return_union_name(&service.name))),
                rust::Type::named(Path::new(&[
                    "zerodds_rpc",
                    "common_types",
                    "RemoteExceptionCode",
                ])),
            ],
        )),
        body: Some(rust::Block {
            statements: vec![rust::Statement::Tail(rust::Expr::Match {
                scrutinee: Box::new(rust::Expr::Path(Path::single("request"))),
                arms,
            })],
        }),
    }))
}

fn dispatcher_impl(ast: &Ast, service: &Service) -> Result<Vec<rust::Item>, RustInterfaceError> {
    let _ = ast;
    let mut generic = dispatcher_generic();
    generic.bounds = service_bound(service);
    let self_type = rust::Type::generic(
        Path::single(&dispatcher_type_name(service)),
        vec![rust::Type::named(Path::single("S"))],
    );

    let new_fn = rust::Item::Function(rust::Function {
        doc: Doc::line("Wraps a service implementation, ready to serve."),
        attrs: Vec::new(),
        visibility: Visibility::Public,
        name: Ident::new("new"),
        generics: Vec::new(),
        params: vec![rust::Param {
            name: Some(Ident::new("service")),
            type_: rust::Type::named(Path::single("S")),
            receiver: None,
        }],
        returns: Some(rust::Type::named(Path::single("Self"))),
        body: Some(rust::Block {
            statements: vec![rust::Statement::Tail(rust::Expr::StructLit {
                path: Path::single("Self"),
                fields: vec![(
                    Ident::new("service"),
                    rust::Expr::Path(Path::single("service")),
                )],
                base: None,
            })],
        }),
    });

    let call_name = interface::call_union_name(&service.name);
    let return_name = interface::return_union_name(&service.name);
    let serve_fn = rust::Item::Function(rust::Function {
        doc: Doc::line(
            "Registers a `Replier` for this interface level and starts serving. \
             Ticking it — reading requests, calling the service, writing replies \
             — is the caller's own event loop, as `zerodds_rpc::replier::Replier` \
             always leaves it.",
        ),
        attrs: Vec::new(),
        visibility: Visibility::Public,
        name: Ident::new("serve"),
        generics: Vec::new(),
        params: vec![
            rust::Param {
                name: None,
                type_: rust::Type::unit(),
                receiver: Some(rust::Receiver::Value),
            },
            rust::Param {
                name: Some(Ident::new("participant")),
                type_: dcps_participant_type(),
                receiver: None,
            },
            rust::Param {
                name: Some(Ident::new("qos")),
                type_: rpc_qos_type(),
                receiver: None,
            },
        ],
        returns: Some(rpc_result_of(rust::Type::generic(
            Path::new(&["zerodds_rpc", "replier", "Replier"]),
            vec![
                rust::Type::named(Path::single(&call_name)),
                rust::Type::named(Path::single(&return_name)),
            ],
        ))),
        body: Some(rust::Block {
            statements: vec![rust::Statement::Tail(rust::Expr::Call {
                callee: Box::new(rust::Expr::Path(Path::new(&[
                    "zerodds_rpc",
                    "replier",
                    "Replier",
                    "with_options",
                ]))),
                args: vec![
                    rust::Expr::Path(Path::single("participant")),
                    rust::Expr::Literal(Literal::string(&service.qualified_name)),
                    rust::Expr::Literal(Literal::string("")),
                    rust::Expr::Path(Path::single("qos")),
                    binding_style_expr(),
                    annotated_topics_expr(service),
                    endpoint_options_expr(ast, service)?,
                    rust::Expr::Call {
                        callee: Box::new(rust::Expr::Path(Path::new(&[
                            "std", "sync", "Arc", "new",
                        ]))),
                        args: vec![rust::Expr::Path(Path::single("self"))],
                    },
                ],
            })],
        }),
    });

    let inherent = rust::Item::Impl(rust::Impl {
        doc: Doc::none(),
        attrs: Vec::new(),
        generics: vec![generic.clone()],
        trait_: None,
        self_type: self_type.clone(),
        items: {
            let mut items = vec![new_fn, serve_fn];
            if let Some(request_profile) = service.qos_request_profile.as_deref() {
                items.push(declared_qos_fn(
                    "serve_with_declared_qos",
                    "Like `serve`, with the QoS `@Qos` declares for this interface (DDS-RPC 1.0 §7.10.1).",
                    request_profile,
                    service.qos_reply_profile.as_deref(),
                    Some(rust::Receiver::Value),
                    rpc_result_of(rust::Type::generic(
                        Path::new(&["zerodds_rpc", "replier", "Replier"]),
                        vec![
                            rust::Type::named(Path::single(&call_name)),
                            rust::Type::named(Path::single(&return_name)),
                        ],
                    )),
                    &["Self", "serve"],
                ));
            }
            items
        },
    });

    let ops = service.all_operations();
    let handler = rust::Item::Impl(rust::Impl {
        doc: Doc::none(),
        attrs: Vec::new(),
        generics: vec![generic],
        trait_: Some(rust::Type::generic(
            Path::new(&["zerodds_rpc", "replier", "ReplierHandler"]),
            vec![
                rust::Type::named(Path::single(&call_name)),
                rust::Type::named(Path::single(&return_name)),
            ],
        )),
        self_type,
        items: vec![dispatcher_handle_fn(service, &ops)?],
    });

    Ok(vec![inherent, handler])
}
