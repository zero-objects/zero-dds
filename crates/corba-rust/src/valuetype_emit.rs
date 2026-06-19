// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL `valuetype` → Rust trait + marshalling.
//!
//! Mapping (CORBA 3.3 §7.4.5.4):
//! - `valuetype V { ... };` → `pub trait V: ValueBase + ...` plus
//!   ValueBase marshalling.
//! - State members (`public T x;`/`private T x;`) → trait method
//!   for read access (public state); encode/decode copies state.
//! - `init` constructors → free function `<v>_init_<name>(...)`.
//! - Inheritance — mapped via a trait bound in `bases`.
//!
//! **Scope**: trait definition (polymorphic interface) + public-state
//! accessors, plus a concrete `<Name>Value` struct as a wire-marshallable
//! state carrier (`ValueBase` + `ValueMarshal`, §15.3.4) including factory
//! registration. Chunked/truncation/custom marshalling are follow-up features.

use zerodds_idl::ast::types::{
    Declarator, Export, InitDcl, StateVisibility, ValueDef, ValueElement,
};

use crate::error::Result;

/// A marshallable state member: field name + Rust type (in declaration order).
#[derive(Clone)]
struct ValueField {
    name: String,
    rust_type: String,
}

/// Stored valuetype state (own fields + base simple names) for the
/// inheritance state flattening (§15.3.4: base state is marshalled BEFORE the
/// derived state).
struct StoredValue {
    fields: Vec<ValueField>,
    bases: Vec<String>,
}

thread_local! {
    /// Value-state registry (simple name → own state + bases), filled by the
    /// emitter before codegen. Thread-local (per codegen run) so parallel runs
    /// stay isolated. Same pattern as `set_exception_repo_ids`.
    static VALUE_STATES: core::cell::RefCell<std::collections::BTreeMap<String, StoredValue>> =
        const { core::cell::RefCell::new(std::collections::BTreeMap::new()) };
}

/// Registers the state layouts of all valuetypes in a spec (for inheritance
/// flattening). Called by the emitter before the first `emit_valuetype`.
///
/// # Errors
/// Type-mapping error of a state member.
pub fn register_value_states(spec: &zerodds_idl::ast::types::Specification) -> Result<()> {
    let mut map = std::collections::BTreeMap::new();
    collect_value_states(&spec.definitions, &mut map)?;
    VALUE_STATES.with(|r| *r.borrow_mut() = map);
    Ok(())
}

/// zerodds-lint: recursion-depth 64 (Valuetype-Inheritance; bounded by IDL nesting)
fn collect_value_states(
    defs: &[zerodds_idl::ast::types::Definition],
    map: &mut std::collections::BTreeMap<String, StoredValue>,
) -> Result<()> {
    use zerodds_idl::ast::types::Definition;
    for def in defs {
        match def {
            Definition::Module(m) => collect_value_states(&m.definitions, map)?,
            Definition::ValueDef(v) => {
                let bases = v
                    .inheritance
                    .as_ref()
                    .map(|inh| {
                        inh.bases
                            .iter()
                            .filter_map(|b| b.parts.last().map(|p| p.text.clone()))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                map.insert(
                    v.name.text.clone(),
                    StoredValue {
                        fields: collect_state_fields(v)?,
                        bases,
                    },
                );
            }
            _ => {}
        }
    }
    Ok(())
}

/// Collects the transitive base RepositoryIds of a valuetype (most-derived's
/// bases first, then their bases) for the chunked/truncatable RepositoryId list.
/// Uses the [`VALUE_STATES`] registry (base simple names) + `scope` for the RepoIds.
fn collect_base_repo_ids(simple_name: &str, scope: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    collect_base_ids_inner(simple_name, scope, &mut seen, &mut out);
    out
}

/// zerodds-lint: recursion-depth 16
fn collect_base_ids_inner(
    simple_name: &str,
    scope: &[&str],
    seen: &mut std::collections::BTreeSet<String>,
    out: &mut Vec<String>,
) {
    let Some(bases) = VALUE_STATES.with(|r| r.borrow().get(simple_name).map(|sv| sv.bases.clone()))
    else {
        return;
    };
    for base in &bases {
        if seen.insert(base.clone()) {
            out.push(zerodds_corba_codegen::build_repository_id(
                scope, base, 1, 0,
            ));
            collect_base_ids_inner(base, scope, seen, out);
        }
    }
}

/// Flattens the state of a valuetype across the base chain: base state first
/// (recursively, in declaration order of the bases), then own state (§15.3.4).
/// Uses the [`VALUE_STATES`] registry. Unknown/abstract bases contribute nothing.
fn flatten_fields(simple_name: &str) -> Vec<ValueField> {
    let mut seen = std::collections::BTreeSet::new();
    flatten_fields_inner(simple_name, &mut seen)
}

/// zerodds-lint: recursion-depth 16
fn flatten_fields_inner(
    simple_name: &str,
    seen: &mut std::collections::BTreeSet<String>,
) -> Vec<ValueField> {
    let mut out = Vec::new();
    if !seen.insert(simple_name.to_string()) {
        return out; // cycle guard (diamond via abstract bases).
    }
    let Some((bases, own)) = VALUE_STATES.with(|r| {
        r.borrow()
            .get(simple_name)
            .map(|sv| (sv.bases.clone(), sv.fields.clone()))
    }) else {
        return out;
    };
    for base in &bases {
        out.extend(flatten_fields_inner(base, seen));
    }
    out.extend(own);
    out
}

/// Collects all state members (public + private) in declaration order.
/// On the wire (§15.3.4) ALL state members are marshalled — visibility is
/// purely a language access question, not a wire question.
fn collect_state_fields(v: &ValueDef) -> Result<Vec<ValueField>> {
    let mut fields = Vec::new();
    for element in &v.elements {
        if let ValueElement::State(sm) = element {
            let rust_type = zerodds_idl_rust::type_map::rust_type_for(&sm.type_spec)?;
            for declarator in &sm.declarators {
                let name = match declarator {
                    Declarator::Simple(n) => n.text.clone(),
                    Declarator::Array(a) => a.name.text.clone(),
                };
                fields.push(ValueField {
                    name,
                    rust_type: rust_type.clone(),
                });
            }
        }
    }
    Ok(fields)
}

/// Emits an IDL valuetype definition as a Rust trait + wire-marshallable struct.
pub fn emit_valuetype(out: &mut String, v: &ValueDef, scope: &[&str]) -> Result<()> {
    let name = &v.name.text;
    out.push_str("/// Generated by `zerodds-corba-rust` from IDL valuetype.\n");

    // §4.2 Inheritance: trait bounds from inheritance.bases.
    let mut bounds =
        String::from("zerodds_corba_rust::ValueBase + ::core::marker::Send + ::core::marker::Sync");
    if let Some(inh) = &v.inheritance {
        for base in &inh.bases {
            let path = base
                .parts
                .iter()
                .map(|p| p.text.as_str())
                .collect::<Vec<_>>()
                .join("::");
            bounds.push_str(&format!(" + {path}"));
        }
    }

    // §6.1 Repository ID as a free pub const (dyn compatibility) —
    // via `zerodds-corba-codegen::build_repository_id` (CORBA 3.3
    // §10.7.3.1 format), scope-correct (module path).
    let repo_id = zerodds_corba_codegen::build_repository_id(scope, name, 1, 0);
    let repo_const = name.to_ascii_uppercase();
    out.push_str(&format!(
        "/// CORBA Repository-ID for valuetype `{name}` (spec §10.7.3.1).\npub const {repo_const}_REPOSITORY_ID: &str = \"{repo_id}\";\n"
    ));
    out.push_str(&format!("pub trait {name}: {bounds} {{\n"));

    for element in &v.elements {
        emit_value_element(out, name, element)?;
    }

    out.push_str("}\n\n");

    // Wire-marshallable state carrier (§15.3.4) — state flattened across the
    // base chain (base state first).
    let fields = flatten_fields(name);
    emit_value_struct(out, name, &repo_const, &fields);

    // Truncatable/custom (§15.3.4.3): base RepositoryId list for the chunked wire.
    // `valuetype D : truncatable B` or `custom valuetype` → a foreign ORB can
    // truncate to a base; for that the list of base RepoIds must be known.
    let truncatable = v.inheritance.as_ref().is_some_and(|inh| inh.truncatable);
    let custom = matches!(v.kind, zerodds_idl::ast::types::ValueKind::Custom);
    if truncatable || custom {
        let base_ids = collect_base_repo_ids(name, scope);
        let list = base_ids
            .iter()
            .map(|id| format!("\"{id}\""))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "/// Base RepositoryIds for chunked/truncatable wire (§15.3.4.3) —\n/// use `ValueWriter::write_chunked(w, value, {repo_const}_BASE_IDS)`.\npub const {repo_const}_BASE_IDS: &[&str] = &[{list}];\n\n"
        ));
    }

    // §4.3 Init constructors as free functions.
    for element in &v.elements {
        if let ValueElement::Init(init) = element {
            emit_init_function(out, name, init)?;
        }
    }

    Ok(())
}

/// Emits the concrete `<Name>Value` struct + `ValueBase` + `ValueMarshal` +
/// factory registration (§15.3.4 state marshalling).
fn emit_value_struct(out: &mut String, name: &str, repo_const: &str, fields: &[ValueField]) {
    let struct_name = format!("{name}Value");

    // Struct with all state members as pub fields.
    out.push_str(&format!(
        "/// Concrete, wire-marshallable state carrier for valuetype `{name}` (§15.3.4).\n"
    ));
    out.push_str("#[derive(Debug, Clone, PartialEq)]\n");
    out.push_str(&format!("pub struct {struct_name} {{\n"));
    for f in fields {
        out.push_str(&format!("    /// State-member `{}`.\n", f.name));
        out.push_str(&format!("    pub {}: {},\n", f.name, f.rust_type));
    }
    out.push_str("}\n\n");

    // ValueBase: RepositoryId.
    out.push_str(&format!(
        "impl zerodds_corba_rust::ValueBase for {struct_name} {{\n"
    ));
    out.push_str("    fn repository_id(&self) -> &str {\n");
    out.push_str(&format!("        {repo_const}_REPOSITORY_ID\n"));
    out.push_str("    }\n}\n\n");

    // ValueMarshal: state in declaration order (§15.3.4).
    out.push_str(&format!(
        "impl zerodds_corba_rust::value_wire::ValueMarshal for {struct_name} {{\n"
    ));
    out.push_str("    fn marshal_state(&self, w: &mut zerodds_cdr::BufferWriter) -> ::core::result::Result<(), zerodds_cdr::EncodeError> {\n");
    for f in fields {
        out.push_str(&format!(
            "        zerodds_cdr::CdrEncode::encode(&self.{}, w)?;\n",
            f.name
        ));
    }
    out.push_str("        ::core::result::Result::Ok(())\n");
    out.push_str("    }\n}\n\n");

    // Factory: reads state, registers itself in a ValueRegistry.
    out.push_str(&format!(
        "/// Registers the ValueFactory for `{name}` in a `ValueRegistry` (§15.3.4 decode).\n"
    ));
    out.push_str(&format!(
        "pub fn register_{}_value(reg: &mut zerodds_corba_rust::value_wire::ValueRegistry) {{\n",
        name.to_lowercase()
    ));
    out.push_str(&format!("    reg.register({repo_const}_REPOSITORY_ID, ::std::boxed::Box::new(|r: &mut zerodds_cdr::BufferReader<'_>| {{\n"));
    for f in fields {
        out.push_str(&format!(
            "        let {0} = <{1} as zerodds_cdr::CdrDecode>::decode(r)?;\n",
            f.name, f.rust_type
        ));
    }
    out.push_str(&format!(
        "        ::core::result::Result::Ok(::std::rc::Rc::new({struct_name} {{\n"
    ));
    for f in fields {
        out.push_str(&format!("            {},\n", f.name));
    }
    out.push_str("        }) as ::std::rc::Rc<dyn ::core::any::Any>)\n");
    out.push_str("    }));\n");
    out.push_str("}\n\n");
}

fn emit_init_function(out: &mut String, value_name: &str, init: &InitDcl) -> Result<()> {
    let init_name = &init.name.text;
    let fn_name = format!("{}_init_{}", value_name.to_lowercase(), init_name);
    let mut params = String::new();
    for (idx, p) in init.params.iter().enumerate() {
        if idx > 0 {
            params.push_str(", ");
        }
        let ty = zerodds_idl_rust::type_map::rust_type_for(&p.type_spec)?;
        params.push_str(&format!("{}: {ty}", p.name.text));
    }
    out.push_str(&format!(
        "/// IDL `factory {init_name}` constructor for valuetype `{value_name}`.\n"
    ));
    out.push_str(
        "/// Phase 2 wires this to the ValueFactory registry; phase 1 is a placeholder.\n",
    );
    out.push_str(&format!(
        "pub fn {fn_name}({params}) -> ::core::result::Result<::std::boxed::Box<dyn {value_name}>, zerodds_corba_rust::CorbaException> {{\n"
    ));
    // Use of params (silence unused warnings)
    for p in &init.params {
        out.push_str(&format!("    let _ = {};\n", p.name.text));
    }
    out.push_str(
        "    ::core::result::Result::Err(zerodds_corba_rust::CorbaException::SystemException {\n",
    );
    out.push_str("        minor: 0,\n");
    out.push_str("        message: \"corba-rust: ValueFactory init not yet wired (Phase-2)\",\n");
    out.push_str("    })\n");
    out.push_str("}\n\n");
    Ok(())
}

fn emit_value_element(out: &mut String, owner: &str, element: &ValueElement) -> Result<()> {
    match element {
        ValueElement::State(sm) => {
            let ty = zerodds_idl_rust::type_map::rust_type_for(&sm.type_spec)?;
            let public = matches!(sm.visibility, StateVisibility::Public);
            for declarator in &sm.declarators {
                let name = match declarator {
                    Declarator::Simple(n) => &n.text,
                    Declarator::Array(a) => &a.name.text,
                };
                let prefix = if public { "" } else { "_priv_" };
                out.push_str(&format!(
                    "    /// State-member `{name}` (public={public}).\n"
                ));
                out.push_str(&format!("    fn {prefix}{name}(&self) -> {ty};\n"));
            }
        }
        ValueElement::Init(_) => {
            // Init constructors are emitted as free functions
            // `<valuetype>_init_<name>(...)` (phase 2).
        }
        ValueElement::Export(Export::Op(op)) => {
            crate::interface_emit::emit_op_trait_method_pub(out, owner, op)?;
        }
        ValueElement::Export(Export::Attr(attr)) => {
            crate::interface_emit::emit_attr_trait_method_pub(out, attr)?;
        }
        ValueElement::Export(_) => {
            // Type/Const/Except exports are emitted in the idl-rust DataType
            // path.
        }
    }
    Ok(())
}
