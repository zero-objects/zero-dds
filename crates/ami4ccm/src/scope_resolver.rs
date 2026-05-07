// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Scope-aware Symbol-Tabellen-Builder fuer den AMI4CCM-Naming-
//! Conflict-Resolver — Spec §7.5 / §7.3.1.
//!
//! Der Resolver in [`crate::transform::resolve_unique_iface_name`]
//! prueft `AMI4CCM_<Iface>` (und `…ReplyHandler`) gegen ein Set
//! "bereits-bekannter Identifier" und schiebt `AMI_`-Prefixe vor, bis
//! der Name eindeutig ist. Bisher wurde dieser Set vom Caller von Hand
//! gefuettert — das Scope-blinde Verhalten war der letzte offene Punkt
//! aus `omg-ami4ccm-1.1.open.md` §7.5.
//!
//! Dieses Modul liest eine [`Specification`] und sammelt **alle**
//! Top-Level-Identifier (Modules, Interfaces, Types, Consts, Excepts,
//! Components, Homes, Events, Porttypes, Connectors, ValueTypes, plus
//! deren Forward-Decls) sowie alle qualifizierten Modul-Identifier in
//! einen [`crate::transform::TransformContext`]. Damit ist der
//! Konflikt-Check spec-konform fuer den **gesamten** Compilation-Scope.
//!
//! Spec-Hintergrund: §7.5 verlangt Eindeutigkeit "in the interface or
//! any of the interface's base interfaces" und im Compilation-Scope.
//! Der Symbol-Walker hier ist die fehlende dritte Quelle (modul-/
//! repository-Scope) neben Iface-Members + base-Iface-Names.

use alloc::format;
use alloc::string::{String, ToString};

use zerodds_idl::ast::{
    ComponentDcl, ConstrTypeDecl, Definition, EventDcl, HomeDcl, Identifier, InterfaceDcl,
    ModuleDef, PorttypeDcl, Specification, StructDcl, TypeDecl, UnionDcl,
};

use crate::transform::TransformContext;

/// Erweitert einen [`TransformContext`] um alle Top-Level-Identifier
/// einer kompletten [`Specification`].
///
/// Nach dem Aufruf enthaelt `ctx.known_symbols` jeden `<name>` aus
/// jedem Modul-Pfad, sowohl als bare Identifier (z.B. `Order`) als
/// auch als vollqualifizierter Pfad (z.B. `Trading::Order`). Der
/// Resolver matcht gegen die Bare-Form, weil die AMI4CCM-Namen ohne
/// Modul-Prefix emittiert werden.
pub fn populate_from_specification(ctx: &mut TransformContext, spec: &Specification) {
    let mut path_prefix = String::new();
    for d in &spec.definitions {
        walk_definition(ctx, d, &mut path_prefix);
    }
}

/// Bequeme Konstruktor-Variante: erzeugt einen frischen Kontext und
/// fuellt ihn aus der `Specification`.
#[must_use]
pub fn context_from_specification(spec: &Specification) -> TransformContext {
    let mut ctx = TransformContext::default();
    populate_from_specification(&mut ctx, spec);
    ctx
}

/// zerodds-lint: recursion-depth 32
///
/// Indirekt rekursiv via `walk_module`. IDL-Module-Verschachtelung ist
/// in der Praxis flach (typisch <5 Ebenen); 32 deckt selbst extreme
/// generated-Codes ab.
fn walk_definition(ctx: &mut TransformContext, d: &Definition, scope: &mut String) {
    match d {
        Definition::Module(m) => {
            insert_with_scope(ctx, scope, &m.name.text);
            walk_module(ctx, m, scope);
        }
        Definition::Type(t) => walk_type_decl(ctx, t, scope),
        Definition::Const(c) => insert_with_scope(ctx, scope, &c.name.text),
        Definition::Except(e) => insert_with_scope(ctx, scope, &e.name.text),
        Definition::Interface(i) => walk_interface(ctx, i, scope),
        Definition::ValueBox(v) => insert_with_scope(ctx, scope, &v.name.text),
        Definition::ValueForward(v) => insert_with_scope(ctx, scope, &v.name.text),
        Definition::ValueDef(v) => insert_with_scope(ctx, scope, &v.name.text),
        Definition::Component(c) => insert_with_scope(ctx, scope, component_name(c).text.as_str()),
        Definition::Home(h) => insert_with_scope(ctx, scope, home_name(h).text.as_str()),
        Definition::Event(e) => insert_with_scope(ctx, scope, event_name(e).text.as_str()),
        Definition::Porttype(p) => insert_with_scope(ctx, scope, porttype_name(p).text.as_str()),
        Definition::Connector(c) => insert_with_scope(ctx, scope, &c.name.text),
        Definition::Annotation(a) => insert_with_scope(ctx, scope, &a.name.text),
        Definition::TemplateModule(tm) => insert_with_scope(ctx, scope, &tm.name.text),
        Definition::TemplateModuleInst(tmi) => {
            insert_with_scope(ctx, scope, &tmi.instance_name.text);
        }
        // Aliases fuer benannte Type-IDs ohne eigenes Symbol.
        Definition::TypeId(_) | Definition::TypePrefix(_) | Definition::Import(_) => {}
        Definition::VendorExtension(_) => {}
    }
}

/// zerodds-lint: recursion-depth 32
///
/// Indirekt rekursiv via `walk_definition` (Module → Module). Selbe
/// Bound wie `walk_definition`.
fn walk_module(ctx: &mut TransformContext, m: &ModuleDef, scope: &mut String) {
    let saved_len = scope.len();
    if !scope.is_empty() {
        scope.push_str("::");
    }
    scope.push_str(&m.name.text);
    for inner in &m.definitions {
        walk_definition(ctx, inner, scope);
    }
    scope.truncate(saved_len);
}

fn walk_type_decl(ctx: &mut TransformContext, t: &TypeDecl, scope: &str) {
    match t {
        TypeDecl::Constr(c) => match c {
            ConstrTypeDecl::Struct(s) => {
                insert_with_scope(ctx, scope, struct_name(s).text.as_str())
            }
            ConstrTypeDecl::Union(u) => insert_with_scope(ctx, scope, union_name(u).text.as_str()),
            ConstrTypeDecl::Enum(e) => insert_with_scope(ctx, scope, &e.name.text),
            ConstrTypeDecl::Bitset(b) => insert_with_scope(ctx, scope, &b.name.text),
            ConstrTypeDecl::Bitmask(b) => insert_with_scope(ctx, scope, &b.name.text),
        },
        TypeDecl::Typedef(td) => {
            for decl in &td.declarators {
                insert_with_scope(ctx, scope, &decl.name().text);
            }
        }
    }
}

fn component_name(c: &ComponentDcl) -> &Identifier {
    match c {
        ComponentDcl::Def(d) => &d.name,
        ComponentDcl::Forward(id, _) => id,
    }
}

fn home_name(h: &HomeDcl) -> &Identifier {
    match h {
        HomeDcl::Def(d) => &d.name,
        HomeDcl::Forward(id, _) => id,
    }
}

fn event_name(e: &EventDcl) -> &Identifier {
    match e {
        EventDcl::Def(d) => &d.name,
        EventDcl::Forward(id, _) => id,
    }
}

fn porttype_name(p: &PorttypeDcl) -> &Identifier {
    match p {
        PorttypeDcl::Def(d) => &d.name,
        PorttypeDcl::Forward(id, _) => id,
    }
}

fn struct_name(s: &StructDcl) -> &Identifier {
    match s {
        StructDcl::Def(d) => &d.name,
        StructDcl::Forward(f) => &f.name,
    }
}

fn union_name(u: &UnionDcl) -> &Identifier {
    match u {
        UnionDcl::Def(d) => &d.name,
        UnionDcl::Forward(f) => &f.name,
    }
}

fn walk_interface(ctx: &mut TransformContext, i: &InterfaceDcl, scope: &str) {
    let name = match i {
        InterfaceDcl::Def(d) => &d.name.text,
        InterfaceDcl::Forward(f) => &f.name.text,
    };
    insert_with_scope(ctx, scope, name);
}

/// Fuegt sowohl die bare Form (`Order`) als auch die vollqualifizierte
/// Form (`Trading::Order`) in `ctx.known_symbols` ein. Der Resolver
/// matcht gegen die bare Form, aber Caller, die mit qualifizierten
/// Namen arbeiten, koennen ebenfalls treffen.
fn insert_with_scope(ctx: &mut TransformContext, scope: &str, name: &str) {
    ctx.add_known_symbol(name);
    if !scope.is_empty() {
        ctx.add_known_symbol(&format!("{scope}::{name}"));
    }
    // AMI4CCM_-Praefix-Variante explizit propagieren, damit der
    // Resolver den Fall "AMI4CCM_<X> existiert bereits als
    // user-defined Symbol" sauber abbildet.
    ctx.add_known_symbol(&format!("AMI4CCM_{name}"));
    if !scope.is_empty() {
        ctx.add_known_symbol(&format!("{scope}::AMI4CCM_{name}"));
    }
    let _ = name.to_string();
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_idl::ast::{InterfaceDef, InterfaceKind};
    use zerodds_idl::errors::Span;

    fn ident(s: &str) -> Identifier {
        Identifier::new(s, Span::SYNTHETIC)
    }

    fn iface_def(name: &str) -> Definition {
        Definition::Interface(InterfaceDcl::Def(InterfaceDef {
            kind: InterfaceKind::Plain,
            name: ident(name),
            bases: alloc::vec::Vec::new(),
            exports: alloc::vec::Vec::new(),
            annotations: alloc::vec::Vec::new(),
            span: Span::SYNTHETIC,
        }))
    }

    fn module_def(name: &str, defs: alloc::vec::Vec<Definition>) -> Definition {
        Definition::Module(ModuleDef {
            name: ident(name),
            definitions: defs,
            annotations: alloc::vec::Vec::new(),
            span: Span::SYNTHETIC,
        })
    }

    fn make_spec(defs: alloc::vec::Vec<Definition>) -> Specification {
        Specification {
            definitions: defs,
            span: Span::SYNTHETIC,
        }
    }

    #[test]
    fn flat_specification_collects_interface_idents() {
        let spec = make_spec(alloc::vec![iface_def("Order")]);
        let ctx = context_from_specification(&spec);
        assert!(ctx.known_symbols.contains("Order"));
        assert!(ctx.known_symbols.contains("AMI4CCM_Order"));
    }

    #[test]
    fn nested_module_paths_are_qualified() {
        let inner = iface_def("Trade");
        let mid = module_def("Inner", alloc::vec![inner]);
        let outer = module_def("Outer", alloc::vec![mid]);
        let spec = make_spec(alloc::vec![outer]);
        let ctx = context_from_specification(&spec);
        // Bare form
        assert!(ctx.known_symbols.contains("Trade"));
        // Qualified form
        assert!(ctx.known_symbols.contains("Outer::Inner::Trade"));
        // AMI4CCM-prefix form (qualifiziert + bare).
        assert!(ctx.known_symbols.contains("AMI4CCM_Trade"));
        assert!(ctx.known_symbols.contains("Outer::Inner::AMI4CCM_Trade"));
        // Modul-Idents selbst sind ebenfalls erfasst.
        assert!(ctx.known_symbols.contains("Outer"));
        assert!(ctx.known_symbols.contains("Inner"));
    }

    #[test]
    fn populate_extends_existing_context_without_clearing() {
        let mut ctx = TransformContext::default();
        ctx.add_known_symbol("PreExisting");
        let spec = make_spec(alloc::vec![iface_def("Order")]);
        populate_from_specification(&mut ctx, &spec);
        assert!(ctx.known_symbols.contains("PreExisting"));
        assert!(ctx.known_symbols.contains("Order"));
    }

    #[test]
    fn full_resolver_uses_scope_for_conflict() {
        // Integration: Spec-driven Resolver findet AMI4CCM_<Order> im
        // Module-Scope (nicht im Iface) und prefixt mit AMI_.
        let spec = make_spec(alloc::vec![module_def(
            "Trading",
            alloc::vec![
                iface_def("Order"),
                iface_def("AMI4CCM_Order"), // user-defined Konflikt!
            ],
        )]);
        let ctx = context_from_specification(&spec);
        // Der Konflikt-Resolver fuer "Order" muss "AMI_AMI4CCM_Order"
        // emittieren, weil "AMI4CCM_Order" im Scope existiert.
        use crate::transform::transform_interface_in_context;
        let order = InterfaceDef {
            kind: InterfaceKind::Plain,
            name: ident("Order"),
            bases: alloc::vec::Vec::new(),
            exports: alloc::vec::Vec::new(),
            annotations: alloc::vec::Vec::new(),
            span: Span::SYNTHETIC,
        };
        let out = transform_interface_in_context(&order, &ctx);
        assert_eq!(out.ami_interface.name.text, "AMI_AMI4CCM_Order");
    }
}
