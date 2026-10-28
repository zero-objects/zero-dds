// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL `valuetype` → Rust trait + marshalling.
//!
//! Mapping (CORBA 3.3 §7.4.5.4):
//! - `valuetype V { ... };` → `pub trait V: ValueBase + ...` plus the
//!   `ValueBase` marshalling.
//! - state members (`public T x;` / `private T x;`) → a trait method
//!   for read access; the wire carries all of them either way.
//! - `factory` initialisers → a free `<v>_init_<name>(...)`.
//! - inheritance → a trait bound, and state flattened base-first.
//!
//! ## The registry that is gone
//!
//! The state layout used to be collected into a `thread_local!` before
//! the first `emit_valuetype`, keyed by simple name, and read back
//! during the emit. Two valuetypes with the same simple name in
//! different modules collided in it silently; a caller that forgot the
//! registration step got an empty state and a struct with no fields,
//! also silently.
//!
//! `TypeKind::Valuetype` carries `bases` as resolved references and
//! `state` as member handles, so the flattening walks the model. There
//! is nothing to register and nothing to forget.

use std::collections::BTreeSet;

use zerodds_ast::{AnyId, StateVisibility, TypeKind, TypeNode, ValuetypeKind};
use zerodds_ast_core::Id;

use crate::emitter::Ctx;
use crate::error::{CorbaRustError, Result};
use crate::type_map::{read_of, rust_type_for, write_of};

/// A marshallable state member: its field name and its Rust type.
struct ValueField {
    name: String,
    rust_type: String,
    /// The declared type, for the marshalling expressions.
    type_id: Id<TypeNode>,
}

/// The type node behind a handle.
fn type_of<'a>(ctx: &Ctx<'a>, id: Id<TypeNode>) -> Result<&'a TypeNode> {
    ctx.ast
        .type_(id)
        .map_err(|_| CorbaRustError::StaleHandle { what: "a type" })
}

/// The state of a valuetype, base state first. §15.3.4.
///
/// zerodds-lint: recursion-depth 16 (valuetype inheritance; cycle-guarded)
fn flatten_fields(
    ctx: &Ctx<'_>,
    id: Id<TypeNode>,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<ValueField>,
) -> Result<()> {
    let node = type_of(ctx, id)?;
    let TypeKind::Valuetype { bases, state, .. } = &node.kind else {
        return Ok(());
    };
    // A diamond through abstract bases would otherwise write one base's
    // state twice.
    if !seen.insert(node.name.clone()) {
        return Ok(());
    }
    let bases: Vec<Id<TypeNode>> = bases
        .iter()
        .filter_map(zerodds_ast_core::Ref::resolved)
        .collect();
    let state = state.clone();
    for base in bases {
        flatten_fields(ctx, base, seen, out)?;
    }
    for member in state {
        let member = ctx
            .ast
            .member(member)
            .map_err(|_| CorbaRustError::StaleHandle {
                what: "a state member",
            })?;
        let type_id = member
            .type_
            .resolved()
            .ok_or_else(|| CorbaRustError::UnresolvedType {
                scoped: member.name.clone(),
            })?;
        out.push(ValueField {
            name: member.name.clone(),
            rust_type: rust_type_for(ctx.ast, &member.type_)?,
            type_id,
        });
    }
    Ok(())
}

/// The repository identifiers of a valuetype's transitive bases,
/// most-derived's bases first, for the chunked/truncatable list.
///
/// zerodds-lint: recursion-depth 16 (valuetype inheritance; cycle-guarded)
fn base_repository_ids(
    ctx: &Ctx<'_>,
    id: Id<TypeNode>,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<String>,
) -> Result<()> {
    let node = type_of(ctx, id)?;
    let TypeKind::Valuetype { bases, .. } = &node.kind else {
        return Ok(());
    };
    let bases: Vec<Id<TypeNode>> = bases
        .iter()
        .filter_map(zerodds_ast_core::Ref::resolved)
        .collect();
    for base in bases {
        let base_node = type_of(ctx, base)?;
        if !seen.insert(base_node.name.clone()) {
            continue;
        }
        out.push(repository_id_of(ctx, base));
        base_repository_ids(ctx, base, seen, out)?;
    }
    Ok(())
}

/// The repository identifier the model holds for a type.
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The callee
/// is `zerodds_corba_codegen::repository_id_of`, a different function
/// in another crate; `dds_bounded_recursion` compares only the last
/// segment of a call path, so the delegation reads as a self-call.
fn repository_id_of(ctx: &Ctx<'_>, id: Id<TypeNode>) -> String {
    let Ok(node) = ctx.ast.type_(id) else {
        return String::new();
    };
    zerodds_corba_codegen::repository_id_of(ctx.ast, AnyId::Type(id, node.node_id))
        .map(|resolved| resolved.id)
        .unwrap_or_else(|| zerodds_corba_codegen::build_repository_id(&[], &node.name, 1, 0))
}

/// Emits one valuetype as a Rust trait plus a wire-marshallable state
/// carrier.
pub fn emit_valuetype(out: &mut String, ctx: &Ctx<'_>, id: Id<TypeNode>) -> Result<()> {
    let node = type_of(ctx, id)?;
    let TypeKind::Valuetype {
        kind,
        bases,
        state,
        initializers,
        ..
    } = &node.kind
    else {
        return Ok(());
    };
    let name = node.name.clone();
    let kind = kind.to_owned();
    let initializers = initializers.clone();
    let state = state.clone();
    let node_any = AnyId::Type(id, node.node_id);
    // Every operation whose parent is this valuetype and which is not
    // one of its initialisers.
    let exported: Vec<Id<zerodds_ast::OperationNode>> = ctx
        .ast
        .operations()
        .filter(|(op_id, op)| op.parent == node_any && !initializers.contains(op_id))
        .map(|(op_id, _)| op_id)
        .collect();
    let base_ids: Vec<Id<TypeNode>> = bases
        .iter()
        .filter_map(zerodds_ast_core::Ref::resolved)
        .collect();

    out.push_str("/// Generated by `zerodds-corba-rust` from IDL valuetype.\n");

    // §4.2 inheritance as trait bounds.
    let mut bounds =
        String::from("zerodds_corba_rust::ValueBase + ::core::marker::Send + ::core::marker::Sync");
    for base in &base_ids {
        bounds.push_str(&format!(" + {}", type_of(ctx, *base)?.name));
    }

    let repo_id = repository_id_of(ctx, id);
    let repo_const = name.to_ascii_uppercase();
    out.push_str(&format!(
        "/// CORBA Repository-ID for valuetype `{name}` (spec §10.7.3.1).\npub const {repo_const}_REPOSITORY_ID: &str = \"{repo_id}\";\n"
    ));
    out.push_str(&format!("pub trait {name}: {bounds} {{\n"));

    // State accessors. Visibility is a language question; the wire
    // carries every state member either way (§15.3.4).
    for member in &state {
        let member = ctx
            .ast
            .member(*member)
            .map_err(|_| CorbaRustError::StaleHandle {
                what: "a state member",
            })?;
        let type_ = rust_type_for(ctx.ast, &member.type_)?;
        let public = member
            .facets
            .corba
            .as_ref()
            .and_then(|corba| corba.state_visibility)
            .is_none_or(|visibility| visibility == StateVisibility::Public);
        let prefix = if public { "" } else { "_priv_" };
        let member_name = &member.name;
        out.push_str(&format!(
            "    /// State-member `{member_name}` (public={public}).\n"
        ));
        out.push_str(&format!(
            "    fn {prefix}{member_name}(&self) -> {type_};\n"
        ));
    }

    // Exported operations — rule (105) puts them in a valuetype body
    // beside the state, and they are `OperationNode`s the same as an
    // interface's, told apart from the initialisers by not being in
    // that list.
    for op_id in &exported {
        crate::interface_emit::emit_op_trait_method(out, ctx, &name, *op_id)?;
    }

    out.push_str("}\n\n");

    // The wire-marshallable state carrier (§15.3.4), state flattened
    // across the base chain.
    let mut fields = Vec::new();
    flatten_fields(ctx, id, &mut BTreeSet::new(), &mut fields)?;
    emit_value_struct(out, ctx, &name, &repo_const, &fields)?;

    // §15.3.4.3: a `truncatable` or `custom` valuetype needs its base
    // identifiers on the wire so a foreign ORB can truncate to a base.
    if matches!(kind, ValuetypeKind::Truncatable | ValuetypeKind::Custom) {
        let mut ids = Vec::new();
        base_repository_ids(ctx, id, &mut BTreeSet::new(), &mut ids)?;
        let list = ids
            .iter()
            .map(|id| format!("\"{id}\""))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "/// Base RepositoryIds for chunked/truncatable wire (§15.3.4.3) —\n/// use `ValueWriter::write_chunked(w, value, {repo_const}_BASE_IDS)`.\npub const {repo_const}_BASE_IDS: &[&str] = &[{list}];\n\n"
        ));
    }

    // §4.3 initialisers as free functions.
    for init in &initializers {
        emit_init_function(out, ctx, &name, *init)?;
    }

    Ok(())
}

/// The concrete `<Name>Value` struct, its `ValueBase`, its
/// `ValueMarshal` and its factory registration. §15.3.4.
fn emit_value_struct(
    out: &mut String,
    ctx: &Ctx<'_>,
    name: &str,
    repo_const: &str,
    fields: &[ValueField],
) -> Result<()> {
    let struct_name = format!("{name}Value");

    out.push_str(&format!(
        "/// Concrete, wire-marshallable state carrier for valuetype `{name}` (§15.3.4).\n"
    ));
    out.push_str("#[derive(Debug, Clone, PartialEq)]\n");
    out.push_str(&format!("pub struct {struct_name} {{\n"));
    for field in fields {
        out.push_str(&format!("    /// State-member `{}`.\n", field.name));
        out.push_str(&format!("    pub {}: {},\n", field.name, field.rust_type));
    }
    out.push_str("}\n\n");

    out.push_str(&format!(
        "impl zerodds_corba_rust::ValueBase for {struct_name} {{\n"
    ));
    out.push_str("    fn repository_id(&self) -> &str {\n");
    out.push_str(&format!("        {repo_const}_REPOSITORY_ID\n"));
    out.push_str("    }\n}\n\n");

    out.push_str(&format!(
        "impl zerodds_corba_rust::value_wire::ValueMarshal for {struct_name} {{\n"
    ));
    out.push_str("    fn marshal_state(&self, w: &mut zerodds_wire::Writer) -> ::core::result::Result<(), zerodds_wire::WireError> {\n");
    for field in fields {
        let reference = zerodds_ast_core::Ref::Resolved(field.type_id);
        let write = write_of(ctx.ast, &reference, "w", &format!("self.{}", field.name))?;
        out.push_str(&format!("        {}\n", write.statement_propagating()));
    }
    out.push_str("        ::core::result::Result::Ok(())\n");
    out.push_str("    }\n}\n\n");

    out.push_str(&format!(
        "/// Registers the ValueFactory for `{name}` in a `ValueRegistry` (§15.3.4 decode).\n"
    ));
    out.push_str(&format!(
        "pub fn register_{}_value(reg: &mut zerodds_corba_rust::value_wire::ValueRegistry) {{\n",
        name.to_lowercase()
    ));
    out.push_str(&format!(
        "    reg.register({repo_const}_REPOSITORY_ID, ::std::boxed::Box::new(|r: &mut zerodds_wire::Reader<'_>| {{\n"
    ));
    for field in fields {
        let reference = zerodds_ast_core::Ref::Resolved(field.type_id);
        let read = read_of(ctx.ast, &reference, "r")?;
        out.push_str(&format!("        let {} = {read}?;\n", field.name));
    }
    out.push_str(&format!(
        "        ::core::result::Result::Ok(::std::rc::Rc::new({struct_name} {{\n"
    ));
    for field in fields {
        out.push_str(&format!("            {},\n", field.name));
    }
    out.push_str("        }) as ::std::rc::Rc<dyn ::core::any::Any>)\n");
    out.push_str("    }));\n");
    out.push_str("}\n\n");
    Ok(())
}

fn emit_init_function(
    out: &mut String,
    ctx: &Ctx<'_>,
    value_name: &str,
    init: Id<zerodds_ast::OperationNode>,
) -> Result<()> {
    let node = ctx
        .ast
        .operation(init)
        .map_err(|_| CorbaRustError::StaleHandle {
            what: "a valuetype initialiser",
        })?;
    let init_name = &node.name;
    let fn_name = format!("{}_init_{}", value_name.to_lowercase(), init_name);
    let mut params = String::new();
    let mut names = Vec::new();
    for (index, param) in node.params.iter().enumerate() {
        let param = ctx
            .ast
            .param(*param)
            .map_err(|_| CorbaRustError::StaleHandle {
                what: "a parameter",
            })?;
        if index > 0 {
            params.push_str(", ");
        }
        let type_ = rust_type_for(ctx.ast, &param.type_)?;
        params.push_str(&format!("{}: {type_}", param.name));
        names.push(param.name.clone());
    }
    out.push_str(&format!(
        "/// IDL `factory {init_name}` constructor for valuetype `{value_name}`.\n"
    ));
    out.push_str(
        "/// The ValueFactory registry is wired by `register_<v>_value`; this\n/// constructor is not, and says so rather than returning a value it did\n/// not build.\n",
    );
    out.push_str(&format!(
        "pub fn {fn_name}({params}) -> ::core::result::Result<::std::boxed::Box<dyn {value_name}>, zerodds_corba_rust::CorbaException> {{\n"
    ));
    for name in &names {
        out.push_str(&format!("    let _ = {name};\n"));
    }
    out.push_str(
        "    ::core::result::Result::Err(zerodds_corba_rust::CorbaException::SystemException {\n",
    );
    out.push_str("        minor: 0,\n");
    out.push_str("        message: \"corba-rust: ValueFactory init not wired\",\n");
    out.push_str("    })\n");
    out.push_str("}\n\n");
    Ok(())
}
