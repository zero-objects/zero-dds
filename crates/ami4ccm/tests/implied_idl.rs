// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The AMI4CCM 1.1 §7.3 and §7.5 implied-IDL transformation, driven from
//! IDL rather than from hand-built syntax nodes.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used, missing_docs)]

use zerodds_ami4ccm::{
    TransformContext, context_from_model, populate_from_model, transform_interface,
    transform_interface_in_context,
};
use zerodds_ast::{Ast, InterfaceKind, InterfaceNode};
use zerodds_ast_core::Id;

fn model(source: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(source);
    assert!(
        !parsed.has_errors(),
        "the fixture does not parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(
        lowered.diagnostics.is_empty(),
        "the fixture does not lower: {:?}",
        lowered.diagnostics
    );
    let mut ast = lowered.ast;
    let _ = ast.resolve_all_by_name();
    ast
}

fn interface_by_name(ast: &Ast, name: &str) -> Id<InterfaceNode> {
    ast.interfaces()
        .find(|(_, node)| node.name == name)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no interface named {name}"))
}

fn op_names(ast: &Ast, interface: Id<InterfaceNode>) -> Vec<String> {
    ast.interface(interface)
        .expect("interface")
        .operations
        .iter()
        .filter_map(|id| ast.operation(*id).ok().map(|node| node.name.clone()))
        .collect()
}

fn param_names(ast: &Ast, interface: Id<InterfaceNode>, operation: &str) -> Vec<String> {
    let id = ast
        .interface(interface)
        .expect("interface")
        .operations
        .iter()
        .copied()
        .find(|id| ast.operation(*id).is_ok_and(|node| node.name == operation))
        .unwrap_or_else(|| panic!("no operation named {operation}"));
    ast.operation(id)
        .expect("operation")
        .params
        .iter()
        .filter_map(|id| ast.param(*id).ok().map(|node| node.name.clone()))
        .collect()
}

// ---------------------------------------------------------------------
// §7.3 — the asynchronous interface.
// ---------------------------------------------------------------------

#[test]
fn both_generated_interfaces_are_local() {
    // §7.3 + Annex A: neither is remotable.
    let mut ast = model("interface I { void ping(); };");
    let interface = interface_by_name(&ast, "I");
    let out = transform_interface(&mut ast, interface).expect("transform");
    for id in [out.ami_interface, out.reply_handler] {
        assert_eq!(
            ast.interface(id).expect("generated").kind,
            InterfaceKind::Local
        );
    }
    assert_eq!(
        ast.interface(out.ami_interface).expect("ami").name,
        "AMI4CCM_I"
    );
    assert_eq!(
        ast.interface(out.reply_handler).expect("handler").name,
        "AMI4CCM_IReplyHandler"
    );
}

#[test]
fn an_operation_becomes_sendc_with_the_handler_first() {
    // §7.3.1.1 (p. 7) — `void sendc_<op>(in handler, in/inout args)`.
    let mut ast = model("interface I { long add(in long a, inout long b, out long c); };");
    let interface = interface_by_name(&ast, "I");
    let out = transform_interface(&mut ast, interface).expect("transform");
    assert_eq!(op_names(&ast, out.ami_interface), ["sendc_add"]);
    // The `out` argument is ignored; `in` and `inout` are carried, all
    // as `in`.
    assert_eq!(
        param_names(&ast, out.ami_interface, "sendc_add"),
        ["ami_handler", "a", "b"]
    );
    let id = ast
        .interface(out.ami_interface)
        .expect("ami")
        .operations
        .first()
        .copied()
        .expect("sendc_add");
    assert!(
        ast.operation(id).expect("sendc_add").return_type.is_none(),
        "an asynchronous operation returns void"
    );
    for param in ast.operation(id).expect("sendc_add").params.clone() {
        assert_eq!(
            ast.param(param).expect("param").direction,
            zerodds_ast::ParamDirection::In
        );
    }
}

#[test]
fn the_handler_parameter_is_an_object_reference_to_the_reply_handler() {
    let mut ast = model("interface I { void ping(); };");
    let interface = interface_by_name(&ast, "I");
    let out = transform_interface(&mut ast, interface).expect("transform");
    let id = ast
        .interface(out.ami_interface)
        .expect("ami")
        .operations
        .first()
        .copied()
        .expect("sendc_ping");
    let param = ast.operation(id).expect("sendc_ping").params[0];
    let type_ = ast
        .param(param)
        .expect("ami_handler")
        .type_
        .resolved()
        .expect("resolved");
    let zerodds_ast::TypeKind::ObjectRef { interface } = &ast.type_(type_).expect("type").kind
    else {
        panic!("the handler parameter is not an object reference");
    };
    assert_eq!(
        interface.as_ref().and_then(|r| r.resolved()),
        Some(out.reply_handler)
    );
}

#[test]
fn an_attribute_becomes_a_sendc_getter_and_setter() {
    // §7.3.1.2 (p. 7).
    let mut ast = model("interface I { attribute long rate; readonly attribute long depth; };");
    let interface = interface_by_name(&ast, "I");
    let out = transform_interface(&mut ast, interface).expect("transform");
    let names = op_names(&ast, out.ami_interface);
    assert_eq!(
        names,
        ["sendc_get_rate", "sendc_set_rate", "sendc_get_depth"],
        "a readonly attribute has no setter"
    );
    assert_eq!(
        param_names(&ast, out.ami_interface, "sendc_set_rate"),
        ["ami_handler", "attr_rate"]
    );
}

#[test]
fn a_sendc_operation_carries_no_raises_clause() {
    // §7.1 (p. 5): the only exception an AMI4CCM operation can throw is
    // the `INV_OBJREF` system exception, and a system exception never
    // appears in an IDL `raises` clause. The user exceptions of the
    // original operation travel through the reply handler's `_excep`
    // operation instead.
    let mut ast = model(
        "exception InvalidSymbol { string sym; }; \
         interface I { void ping() raises (InvalidSymbol); };",
    );
    let interface = interface_by_name(&ast, "I");
    let out = transform_interface(&mut ast, interface).expect("transform");
    let id = ast
        .interface(out.ami_interface)
        .expect("ami")
        .operations
        .first()
        .copied()
        .expect("sendc_ping");
    assert!(
        ast.operation(id).expect("sendc_ping").raises.is_empty(),
        "the asynchronous operation must name no user exception"
    );
    assert_eq!(
        param_names(&ast, out.reply_handler, "ping_excep"),
        ["excep_holder"]
    );
}

#[test]
fn a_sendc_name_that_already_exists_gains_an_ami_infix() {
    // §7.3.1 (p. 7).
    let mut ast = model("interface I { void ping(); void sendc_ping(); };");
    let interface = interface_by_name(&ast, "I");
    let out = transform_interface(&mut ast, interface).expect("transform");
    let names = op_names(&ast, out.ami_interface);
    assert!(
        names.contains(&"sendc_ami_ping".to_string()),
        "expected the escalated name, got {names:?}"
    );
}

// ---------------------------------------------------------------------
// §7.5 — the reply handler.
// ---------------------------------------------------------------------

#[test]
fn a_reply_handler_carries_a_normal_and_an_excep_operation_per_operation() {
    // §7.5.1 + §7.5.2 (p. 9-10).
    let mut ast = model("interface I { long add(in long a, inout long b, out long c); };");
    let interface = interface_by_name(&ast, "I");
    let out = transform_interface(&mut ast, interface).expect("transform");
    assert_eq!(op_names(&ast, out.reply_handler), ["add", "add_excep"]);
    // The return value first, then every inout and out argument; the
    // plain `in` argument is not carried back.
    assert_eq!(
        param_names(&ast, out.reply_handler, "add"),
        ["ami_return_val", "b", "c"]
    );
    assert_eq!(
        param_names(&ast, out.reply_handler, "add_excep"),
        ["excep_holder"]
    );
}

#[test]
fn a_void_operation_has_no_return_parameter_in_its_reply() {
    let mut ast = model("interface I { void ping(); };");
    let interface = interface_by_name(&ast, "I");
    let out = transform_interface(&mut ast, interface).expect("transform");
    assert!(param_names(&ast, out.reply_handler, "ping").is_empty());
}

#[test]
fn a_setter_acknowledgement_takes_no_arguments() {
    // §7.5.1 (p. 10).
    let mut ast = model("interface I { attribute long rate; };");
    let interface = interface_by_name(&ast, "I");
    let out = transform_interface(&mut ast, interface).expect("transform");
    assert_eq!(
        op_names(&ast, out.reply_handler),
        ["get_rate", "get_rate_excep", "set_rate", "set_rate_excep"]
    );
    assert_eq!(
        param_names(&ast, out.reply_handler, "get_rate"),
        ["ami_return_val"]
    );
    assert!(param_names(&ast, out.reply_handler, "set_rate").is_empty());
}

#[test]
fn an_excep_name_that_already_exists_gains_an_ami_infix() {
    // §7.5.2 (p. 10) — `_ami` before `_excep`, not `ami_` after it.
    let mut ast = model("interface I { void ping(); void ping_excep(); };");
    let interface = interface_by_name(&ast, "I");
    let out = transform_interface(&mut ast, interface).expect("transform");
    let names = op_names(&ast, out.reply_handler);
    assert!(
        names.contains(&"ping_ami_excep".to_string()),
        "expected the escalated name, got {names:?}"
    );
}

#[test]
fn a_reply_handler_inherits_the_generic_one_by_default() {
    let mut ast = model("interface I { void ping(); };");
    let interface = interface_by_name(&ast, "I");
    let out = transform_interface(&mut ast, interface).expect("transform");
    let bases = ast
        .interface(out.reply_handler)
        .expect("handler")
        .bases
        .clone();
    let zerodds_ast_core::Ref::Pending(zerodds_ast_core::PendingRef::ScopedName(name)) = &bases[0]
    else {
        panic!("the handler base resolved to something");
    };
    assert_eq!(name.segments(), ["CCM_AMI", "ReplyHandler"]);
}

#[test]
fn a_derived_interface_inherits_its_bases_reply_handler() {
    // §7.5 (p. 9).
    let mut ast = model("interface Base { void a(); }; interface Derived : Base { void b(); };");
    let base = interface_by_name(&ast, "Base");
    let mut ctx = TransformContext::new();
    let _ = transform_interface_in_context(&mut ast, base, &ctx).expect("base");
    ctx.mark_transformed("Base");

    let derived = interface_by_name(&ast, "Derived");
    let out = transform_interface_in_context(&mut ast, derived, &ctx).expect("derived");
    let bases = ast
        .interface(out.reply_handler)
        .expect("handler")
        .bases
        .clone();
    let zerodds_ast_core::Ref::Pending(zerodds_ast_core::PendingRef::ScopedName(name)) = &bases[0]
    else {
        panic!("the handler base resolved to something");
    };
    assert_eq!(name.segments(), ["AMI4CCM_BaseReplyHandler"]);
}

// ---------------------------------------------------------------------
// §7.5 / §7.3.1 — the compilation-scope symbol table.
// ---------------------------------------------------------------------

#[test]
fn the_symbol_table_holds_every_declaration_bare_and_qualified() {
    let ast = model("module Outer { module Inner { interface Trade {}; }; };");
    let ctx = context_from_model(&ast);
    assert!(ctx.known_symbols.contains("Trade"));
    assert!(ctx.known_symbols.contains("Outer::Inner::Trade"));
    assert!(ctx.known_symbols.contains("AMI4CCM_Trade"));
    assert!(ctx.known_symbols.contains("Outer::Inner::AMI4CCM_Trade"));
    assert!(ctx.known_symbols.contains("Outer"));
    assert!(ctx.known_symbols.contains("Inner"));
}

#[test]
fn populating_extends_a_context_rather_than_replacing_it() {
    let ast = model("interface Order {};");
    let mut ctx = TransformContext::new();
    ctx.add_known_symbol("PreExisting");
    populate_from_model(&mut ctx, &ast);
    assert!(ctx.known_symbols.contains("PreExisting"));
    assert!(ctx.known_symbols.contains("Order"));
}

#[test]
fn a_user_declared_ami4ccm_name_pushes_the_generated_one_aside() {
    // §7.3.1 / §7.5: the conflict is with the whole compilation scope,
    // not only with the interface's own members.
    let mut ast = model("module Trading { interface Order {}; interface AMI4CCM_Order {}; };");
    let ctx = context_from_model(&ast);
    let order = interface_by_name(&ast, "Order");
    let out = transform_interface_in_context(&mut ast, order, &ctx).expect("transform");
    assert_eq!(
        ast.interface(out.ami_interface).expect("ami").name,
        "AMI_AMI4CCM_Order"
    );
}

#[test]
fn an_ami4ccm_name_conflict_escalates_until_the_name_is_unique() {
    // §7.3.1 / §7.5: the `AMI_` prefix is inserted repeatedly, not once
    // — with both `AMI4CCM_Order` and `AMI_AMI4CCM_Order` already taken
    // the generated name has to escalate a second time.
    let mut ast = model(
        "module Trading { interface Order {}; interface AMI4CCM_Order {}; \
         interface AMI_AMI4CCM_Order {}; };",
    );
    let ctx = context_from_model(&ast);
    let order = interface_by_name(&ast, "Order");
    let out = transform_interface_in_context(&mut ast, order, &ctx).expect("transform");
    assert_eq!(
        ast.interface(out.ami_interface).expect("ami").name,
        "AMI_AMI_AMI4CCM_Order"
    );
}

// ---------------------------------------------------------------------
// §7.2 / §7.3.1.3 / §7.5.3 — the running example.
// ---------------------------------------------------------------------

/// The `StockManager` interface of the §7.2 running example: one
/// attribute plus the five operations whose implied-IDL signatures
/// §7.3.1.3 and §7.5.3 list.
const STOCK_MANAGER_IDL: &str = "
module Stock {
  interface StockManager {
    attribute string stock_exchange_name;
    void set_stock(in string symbol, in double value);
    boolean remove_stock(in string symbol);
    string find_closest_symbol(in string symbol, out double value);
    double get_quote(in string symbol, inout string full_name);
  };
};
";

#[test]
fn the_running_example_yields_the_sendc_operations_of_section_7_3_1_3() {
    // §7.3.1.3 (p. 8) — the expected `sendc_` signature list.
    let mut ast = model(STOCK_MANAGER_IDL);
    let manager = interface_by_name(&ast, "StockManager");
    let out = transform_interface(&mut ast, manager).expect("transform");

    // The emitter groups the operation-derived names before the
    // attribute-derived ones; IDL does not make member order normative,
    // so the comparison is over the set the spec lists.
    let mut emitted = op_names(&ast, out.ami_interface);
    emitted.sort();
    let mut expected = vec![
        "sendc_get_stock_exchange_name",
        "sendc_set_stock_exchange_name",
        "sendc_set_stock",
        "sendc_remove_stock",
        "sendc_find_closest_symbol",
        "sendc_get_quote",
    ];
    expected.sort_unstable();
    assert_eq!(emitted, expected);
    // §7.3.1.1: the handler comes first, `in` and `inout` follow, `out`
    // is dropped.
    assert_eq!(
        param_names(&ast, out.ami_interface, "sendc_set_stock"),
        ["ami_handler", "symbol", "value"]
    );
    assert_eq!(
        param_names(&ast, out.ami_interface, "sendc_find_closest_symbol"),
        ["ami_handler", "symbol"]
    );
    assert_eq!(
        param_names(&ast, out.ami_interface, "sendc_get_quote"),
        ["ami_handler", "symbol", "full_name"]
    );
    // §7.3.1.2: the setter argument carries the `attr_` prefix.
    assert_eq!(
        param_names(&ast, out.ami_interface, "sendc_set_stock_exchange_name"),
        ["ami_handler", "attr_stock_exchange_name"]
    );
}

#[test]
fn the_running_example_yields_the_reply_handler_of_section_7_5_3() {
    // §7.5.3 (p. 10-11) — the expected ReplyHandler operation list.
    let mut ast = model(STOCK_MANAGER_IDL);
    let manager = interface_by_name(&ast, "StockManager");
    let out = transform_interface(&mut ast, manager).expect("transform");

    let mut emitted = op_names(&ast, out.reply_handler);
    emitted.sort();
    let mut expected = vec![
        "get_stock_exchange_name",
        "get_stock_exchange_name_excep",
        "set_stock_exchange_name",
        "set_stock_exchange_name_excep",
        "set_stock",
        "set_stock_excep",
        "remove_stock",
        "remove_stock_excep",
        "find_closest_symbol",
        "find_closest_symbol_excep",
        "get_quote",
        "get_quote_excep",
    ];
    expected.sort_unstable();
    assert_eq!(emitted, expected);
    // §7.5.1: the return value first, then every `inout`/`out`.
    assert_eq!(
        param_names(&ast, out.reply_handler, "find_closest_symbol"),
        ["ami_return_val", "value"]
    );
    assert_eq!(
        param_names(&ast, out.reply_handler, "get_quote"),
        ["ami_return_val", "full_name"]
    );
    // §7.5.1: a setter acknowledgement carries no argument.
    assert!(param_names(&ast, out.reply_handler, "set_stock_exchange_name").is_empty());
    // §7.5.2: every `_excep` operation takes the ExceptionHolder.
    for operation in [
        "get_stock_exchange_name_excep",
        "set_stock_excep",
        "get_quote_excep",
    ] {
        assert_eq!(
            param_names(&ast, out.reply_handler, operation),
            ["excep_holder"]
        );
    }
}

#[test]
fn the_generated_interfaces_land_in_the_module_the_original_is_in() {
    let mut ast = model("module Trading { interface Order { void place(); }; };");
    let order = interface_by_name(&ast, "Order");
    let module = ast.interface(order).expect("Order").module;
    let out = transform_interface(&mut ast, order).expect("transform");
    assert_eq!(
        ast.interface(out.ami_interface).expect("ami").module,
        module
    );
    assert_eq!(
        ast.interface(out.reply_handler).expect("handler").module,
        module
    );
}
