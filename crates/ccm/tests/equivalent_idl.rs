// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The CCM 4.0 §6 equivalent-IDL transformation, driven from IDL.
//!
//! The old tests hand-built syntax nodes and asserted against them,
//! which could only ever check that the transformation agreed with the
//! test's idea of a component. These start from the IDL the
//! specification writes and go through the real frontend, so a
//! disagreement between the frontend and the transformation is visible
//! here rather than nowhere.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used, missing_docs)]

use zerodds_ast::{Ast, InterfaceKind, InterfaceNode, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ccm::{
    ComponentEquivalent, InitOp, LightweightFilterError, PrimaryKeyError, filter_to_lightweight,
    transform_component, transform_event_type, transform_home, validate_primary_key,
};

/// Parse and lower one file, then resolve what it can.
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
    // `Components::*` is not in the fixture, so names that reach nothing
    // are expected here; the transformation carries them as pending
    // references and this pass does not have to succeed.
    let _ = ast.resolve_all_by_name();
    ast
}

fn interface_by_name(ast: &Ast, name: &str) -> Id<InterfaceNode> {
    ast.interfaces()
        .find(|(_, node)| node.name == name)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no interface named {name}"))
}

fn type_by_name(ast: &Ast, name: &str) -> Id<TypeNode> {
    ast.types()
        .find(|(_, node)| node.name == name)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no type named {name}"))
}

fn op_names(ast: &Ast, interface: Id<InterfaceNode>) -> Vec<String> {
    ast.interface(interface)
        .expect("interface")
        .operations
        .iter()
        .filter_map(|id| ast.operation(*id).ok().map(|node| node.name.clone()))
        .collect()
}

fn base_names(ast: &Ast, interface: Id<InterfaceNode>) -> Vec<String> {
    ast.interface(interface)
        .expect("interface")
        .bases
        .iter()
        .map(|base| reference_name(ast, base))
        .collect()
}

fn reference_name(ast: &Ast, reference: &zerodds_ast_core::Ref<InterfaceNode>) -> String {
    match reference {
        zerodds_ast_core::Ref::Resolved(id) => ast
            .interface(*id)
            .map(|node| node.name.clone())
            .unwrap_or_default(),
        zerodds_ast_core::Ref::Pending(zerodds_ast_core::PendingRef::ScopedName(name)) => {
            name.segments().join("::")
        }
        _ => String::new(),
    }
}

fn return_name(ast: &Ast, interface: Id<InterfaceNode>, operation: &str) -> String {
    let id = ast
        .interface(interface)
        .expect("interface")
        .operations
        .iter()
        .copied()
        .find(|id| ast.operation(*id).is_ok_and(|node| node.name == operation))
        .unwrap_or_else(|| panic!("no operation named {operation}"));
    let returns = ast
        .operation(id)
        .expect("operation")
        .return_type
        .clone()
        .unwrap_or_else(|| panic!("{operation} returns void"));
    match returns {
        zerodds_ast_core::Ref::Resolved(id) => ast
            .type_(id)
            .map(|node| node.name.clone())
            .unwrap_or_default(),
        zerodds_ast_core::Ref::Pending(zerodds_ast_core::PendingRef::ScopedName(name)) => {
            name.segments().join("::")
        }
        _ => String::new(),
    }
}

// ---------------------------------------------------------------------
// §6.3.2 — the equivalent interface's inheritance.
// ---------------------------------------------------------------------

#[test]
fn a_plain_component_inherits_ccmobject() {
    // §6.3.2.1 (p. 12) — `component C {};` →
    // `interface C : Components::CCMObject {};`
    let mut ast = model("component C {};");
    let component = interface_by_name(&ast, "C");
    let out = transform_component(&mut ast, component).expect("transform");
    assert_eq!(
        ast.interface(out.equivalent_interface).expect("C").kind,
        InterfaceKind::Interface,
        "the component is still a component"
    );
    assert_eq!(base_names(&ast, component), ["Components::CCMObject"]);
}

#[test]
fn a_component_with_supports_inherits_ccmobject_and_the_supported_ones() {
    // §6.3.2.2 (p. 12) — `component C supports I1, I2 {};` →
    // `interface C : Components::CCMObject, I1, I2 {};`
    // This is the case the frontend used to drop outright: only the
    // first inheritance clause was read, so `supports` never arrived.
    let mut ast = model("interface I1 {}; interface I2 {}; component C supports I1, I2 {};");
    let component = interface_by_name(&ast, "C");
    let _ = transform_component(&mut ast, component).expect("transform");
    assert_eq!(
        base_names(&ast, component),
        ["Components::CCMObject", "I1", "I2"]
    );
}

#[test]
fn a_derived_component_inherits_its_base_and_not_ccmobject() {
    // §6.3.2.3 (p. 12) — `component C : B {};` → `interface C : B`,
    // because `CCMObject` is transitive through B.
    let mut ast = model("component B {}; component C : B {};");
    let component = interface_by_name(&ast, "C");
    let _ = transform_component(&mut ast, component).expect("transform");
    assert_eq!(base_names(&ast, component), ["B"]);
}

#[test]
fn a_derived_component_with_supports_lists_the_base_first_and_then_the_supported() {
    // §6.3.2.4 (p. 13) — `component C : B supports I1, I2 {};` →
    // `interface C : B, I1, I2 {};`. Neither of the two cases above
    // reaches this one: the base replaces `Components::CCMObject`
    // *and* the supported interfaces follow it, in that order.
    let mut ast = model(
        "interface I1 {}; interface I2 {}; component B {}; component C : B supports I1, I2 {};",
    );
    let component = interface_by_name(&ast, "C");
    let _ = transform_component(&mut ast, component).expect("transform");
    assert_eq!(base_names(&ast, component), ["B", "I1", "I2"]);
    assert!(
        !base_names(&ast, component).contains(&"Components::CCMObject".to_string()),
        "CCMObject is transitive through B and must not be repeated"
    );
}

#[test]
fn a_basic_component_carries_its_attributes_and_no_port_operation() {
    // §6.3.1 (p. 11) — the basic form is `"component" <identifier>
    // [<supported_interface_spec>] "{" {<attr_dcl> ";"}* "}"`:
    // attributes and nothing else. "Basic components are not allowed to
    // offer facets, receptacles, event sources and sinks. They may only
    // offer attributes." (§6.1.1, p. 9.) Nothing in that body maps onto
    // a port operation, so the equivalent interface of a basic
    // component has none.
    let mut ast = model(
        "interface I1 {};
         component C supports I1 { attribute long rate; readonly attribute string name; };",
    );
    let component = interface_by_name(&ast, "C");
    let _ = transform_component(&mut ast, component).expect("transform");
    assert!(
        op_names(&ast, component).is_empty(),
        "a basic component has no ports and must gain no port operations, got {:?}",
        op_names(&ast, component)
    );
    assert_eq!(
        ast.interface(component).expect("C").attributes.len(),
        2,
        "both attributes belong on the equivalent interface"
    );
    assert_eq!(
        base_names(&ast, component),
        ["Components::CCMObject", "I1"],
        "the supported interface of the basic form is still carried"
    );
}

// ---------------------------------------------------------------------
// §6.4 to §6.6 — the port mappings.
// ---------------------------------------------------------------------

#[test]
fn provides_yields_a_provide_operation_returning_the_facet() {
    // §6.4.1 (p. 13) — `provides I foo;` → `I provide_foo();`
    let mut ast = model("interface I {}; component C { provides I foo; };");
    let component = interface_by_name(&ast, "C");
    let _ = transform_component(&mut ast, component).expect("transform");
    assert!(op_names(&ast, component).contains(&"provide_foo".to_string()));
    assert_eq!(return_name(&ast, component, "provide_foo"), "I");
    assert!(
        ast.interface(component).expect("C").attributes.is_empty(),
        "the port survived as an attribute as well as an operation"
    );
}

#[test]
fn uses_simplex_yields_connect_disconnect_and_get_connection() {
    // §6.5.1 (p. 19).
    let mut ast = model("interface I {}; component C { uses I manager; };");
    let component = interface_by_name(&ast, "C");
    let _ = transform_component(&mut ast, component).expect("transform");
    let names = op_names(&ast, component);
    for expected in [
        "connect_manager",
        "disconnect_manager",
        "get_connection_manager",
    ] {
        assert!(names.contains(&expected.to_string()), "missing {expected}");
    }
}

#[test]
fn uses_multiple_returns_a_cookie_and_the_plural_getter() {
    // §6.5.1 (p. 19-20) — a multiplex receptacle's `connect` returns a
    // `Components::Cookie`.
    let mut ast = model("interface I {}; component C { uses multiple I managers; };");
    let component = interface_by_name(&ast, "C");
    let _ = transform_component(&mut ast, component).expect("transform");
    let names = op_names(&ast, component);
    assert!(names.contains(&"get_connections_managers".to_string()));
    assert_eq!(
        return_name(&ast, component, "connect_managers"),
        "Components::Cookie"
    );
}

#[test]
fn emits_yields_connect_and_disconnect_and_the_consumer_interface() {
    // §6.6.6.1 (p. 28).
    let mut ast = model("eventtype Tick {}; component C { emits Tick ticker; };");
    let component = interface_by_name(&ast, "C");
    let out = transform_component(&mut ast, component).expect("transform");
    let names = op_names(&ast, component);
    assert!(names.contains(&"connect_ticker".to_string()));
    assert!(names.contains(&"disconnect_ticker".to_string()));
    assert_eq!(out.event_consumer_interfaces.len(), 1);
    assert_eq!(
        ast.interface(out.event_consumer_interfaces[0])
            .expect("consumer")
            .name,
        "TickConsumer"
    );
}

#[test]
fn publishes_yields_subscribe_returning_a_cookie() {
    // §6.6.5.1 (p. 27).
    let mut ast = model("eventtype Tick {}; component C { publishes Tick ticker; };");
    let component = interface_by_name(&ast, "C");
    let _ = transform_component(&mut ast, component).expect("transform");
    let names = op_names(&ast, component);
    assert!(names.contains(&"subscribe_ticker".to_string()));
    assert!(names.contains(&"unsubscribe_ticker".to_string()));
    assert_eq!(
        return_name(&ast, component, "subscribe_ticker"),
        "Components::Cookie"
    );
}

#[test]
fn consumes_yields_a_get_consumer_operation() {
    // §6.6.7.1 (p. 29) — `consumes Tick sink;` →
    // `TickConsumer get_consumer_sink();`
    let mut ast = model("eventtype Tick {}; component C { consumes Tick sink; };");
    let component = interface_by_name(&ast, "C");
    let _ = transform_component(&mut ast, component).expect("transform");
    assert_eq!(
        return_name(&ast, component, "get_consumer_sink"),
        "TickConsumer"
    );
}

#[test]
fn one_event_type_yields_one_consumer_interface_however_many_ports_name_it() {
    // §6.6 — one consumer interface per event type, not per port.
    let mut ast = model(
        "eventtype Tick {};
         component C { emits Tick a; publishes Tick b; consumes Tick c; };",
    );
    let component = interface_by_name(&ast, "C");
    let out = transform_component(&mut ast, component).expect("transform");
    assert_eq!(out.event_consumer_interfaces.len(), 1);
}

#[test]
fn a_component_attribute_is_left_alone() {
    // An attribute is not a port: §6.3.2 carries it into the equivalent
    // interface unchanged.
    let mut ast = model("component C { attribute long rate; };");
    let component = interface_by_name(&ast, "C");
    let _ = transform_component(&mut ast, component).expect("transform");
    let attributes = ast.interface(component).expect("C").attributes.clone();
    assert_eq!(attributes.len(), 1);
    assert_eq!(ast.attribute(attributes[0]).expect("rate").name, "rate");
}

#[test]
fn the_whole_trader_component_yields_every_operation_the_clauses_name() {
    let mut ast = model(
        "interface StockManager {}; interface Bank {}; interface Feed {};
         eventtype Tick {}; eventtype Order {};
         component Trader {
           provides StockManager manager;
           uses Bank bank;
           uses multiple Feed feeds;
           emits Tick ticker;
           publishes Tick public_ticker;
           consumes Order order_sink;
         };",
    );
    let component = interface_by_name(&ast, "Trader");
    let out = transform_component(&mut ast, component).expect("transform");
    let names = op_names(&ast, component);
    for expected in [
        "provide_manager",
        "connect_bank",
        "disconnect_bank",
        "get_connection_bank",
        "connect_feeds",
        "disconnect_feeds",
        "get_connections_feeds",
        "connect_ticker",
        "disconnect_ticker",
        "subscribe_public_ticker",
        "unsubscribe_public_ticker",
        "get_consumer_order_sink",
    ] {
        assert!(
            names.contains(&expected.to_string()),
            "missing {expected} in {names:?}"
        );
    }
    let consumers: Vec<String> = out
        .event_consumer_interfaces
        .iter()
        .map(|id| ast.interface(*id).expect("consumer").name.clone())
        .collect();
    assert!(consumers.contains(&"TickConsumer".to_string()));
    assert!(consumers.contains(&"OrderConsumer".to_string()));
}

// ---------------------------------------------------------------------
// §6.7 — homes.
// ---------------------------------------------------------------------

#[test]
fn a_keyless_home_yields_an_implicit_inheriting_keylesscmhome() {
    // §6.7.1.1 (p. 33).
    let mut ast = model("component C {}; home CManager manages C {};");
    let home = interface_by_name(&ast, "CManager");
    let out = transform_home(&mut ast, home).expect("transform");
    assert_eq!(
        ast.interface(out.explicit).expect("explicit").name,
        "CManagerExplicit"
    );
    assert_eq!(
        ast.interface(out.implicit).expect("implicit").name,
        "CManagerImplicit"
    );
    assert_eq!(base_names(&ast, out.explicit), ["Components::CCMHome"]);
    assert_eq!(
        base_names(&ast, out.implicit),
        ["Components::KeylessCCMHome"]
    );
    assert_eq!(op_names(&ast, out.implicit), ["create"]);
    assert_eq!(
        base_names(&ast, out.equivalent),
        ["CManagerExplicit", "CManagerImplicit"]
    );
}

#[test]
fn a_home_with_supports_puts_the_supported_interfaces_on_its_explicit() {
    // §6.7.1.3 (p. 35) — `home <h> supports <I> manages <C> {...};` →
    // `interface <h>Explicit : Components::CCMHome, <I> {...};`. The
    // supported interfaces land on the explicit interface, not on the
    // implicit one and not on the equivalent home.
    let mut ast = model(
        "interface Audit {}; interface Admin {}; component C {};
         home CManager supports Audit, Admin manages C {};",
    );
    let home = interface_by_name(&ast, "CManager");
    let out = transform_home(&mut ast, home).expect("transform");
    assert_eq!(
        base_names(&ast, out.explicit),
        ["Components::CCMHome", "Audit", "Admin"]
    );
    assert_eq!(
        base_names(&ast, out.implicit),
        ["Components::KeylessCCMHome"],
        "the implicit interface takes no supported interface"
    );
    assert_eq!(
        base_names(&ast, out.equivalent),
        ["CManagerExplicit", "CManagerImplicit"]
    );
}

#[test]
fn a_keyed_home_yields_the_four_operations_and_inherits_nothing() {
    // §6.7.1.2 (p. 34).
    let mut ast = model(
        "component C {}; valuetype CKey { public long v; };
         home CManager manages C primarykey CKey {};",
    );
    let home = interface_by_name(&ast, "CManager");
    let out = transform_home(&mut ast, home).expect("transform");
    let names = op_names(&ast, out.implicit);
    for expected in ["create", "find_by_primary_key", "remove", "get_primary_key"] {
        assert!(names.contains(&expected.to_string()), "missing {expected}");
    }
    assert!(
        ast.interface(out.implicit)
            .expect("implicit")
            .bases
            .is_empty(),
        "the keyed Implicit interface inherits nothing"
    );
}

#[test]
fn a_derived_home_inherits_the_base_homes_explicit_interface() {
    // §6.7.4 (p. 37): "Each Explicit Home, derived from another home,
    // MUST inherit from the parent's Explicit Interface."
    let mut ast = model(
        "component C {};
         home CManagerBase manages C {};
         home CManagerExt : CManagerBase manages C {};",
    );
    let home = interface_by_name(&ast, "CManagerExt");
    let out = transform_home(&mut ast, home).expect("transform");
    assert_eq!(base_names(&ast, out.explicit), ["CManagerBaseExplicit"]);
}

#[test]
fn a_derived_home_with_supports_extends_the_base_explicit() {
    let mut ast = model(
        "interface IExtra {}; component C {};
         home CManagerBase manages C {};
         home CManagerExt : CManagerBase supports IExtra manages C {};",
    );
    let home = interface_by_name(&ast, "CManagerExt");
    let out = transform_home(&mut ast, home).expect("transform");
    assert_eq!(
        base_names(&ast, out.explicit),
        ["CManagerBaseExplicit", "IExtra"]
    );
}

// ---------------------------------------------------------------------
// §6.6.1.1 — event types.
// ---------------------------------------------------------------------

#[test]
fn the_first_event_type_of_a_chain_inherits_event_base() {
    let mut ast = model("eventtype Tick {};");
    let event = type_by_name(&ast, "Tick");
    let out = transform_event_type(&mut ast, event).expect("transform");
    let TypeKind::Valuetype { bases, .. } = &ast.type_(out.valuetype).expect("Tick").kind else {
        panic!("Tick is not a valuetype");
    };
    assert_eq!(bases.len(), 1, "EventBase was not added");
    assert_eq!(
        ast.interface(out.consumer_interface)
            .expect("consumer")
            .name,
        "TickConsumer"
    );
    assert_eq!(op_names(&ast, out.consumer_interface), ["push_Tick"]);
    let params = ast
        .interface(out.consumer_interface)
        .expect("consumer")
        .operations
        .first()
        .copied()
        .and_then(|id| ast.operation(id).ok())
        .map(|node| node.params.clone())
        .expect("push operation");
    assert_eq!(ast.param(params[0]).expect("param").name, "the_tick");
}

#[test]
fn a_consumer_interface_inherits_event_consumer_base_at_the_root_of_the_chain() {
    // §6.6.2 (p. 25) — `Components::EventConsumerBase` is the base of
    // every event consumer interface: "interface <event_type>Consumer :
    // Components::EventConsumerBase". A derived event type reaches it
    // through the consumer of its own base, which is what keeps the
    // consumer chain parallel to the event chain (§6.6.1.1).
    let mut ast = model("eventtype Tick {}; eventtype PricedTick : Tick {};");

    let root = type_by_name(&ast, "Tick");
    let root_out = transform_event_type(&mut ast, root).expect("transform Tick");
    assert_eq!(
        base_names(&ast, root_out.consumer_interface),
        ["Components::EventConsumerBase"]
    );

    let derived = type_by_name(&ast, "PricedTick");
    let derived_out = transform_event_type(&mut ast, derived).expect("transform PricedTick");
    assert_eq!(
        base_names(&ast, derived_out.consumer_interface),
        ["TickConsumer"],
        "the derived consumer reaches EventConsumerBase through TickConsumer"
    );
}

#[test]
fn a_plain_valuetype_is_not_an_event_type() {
    // The keyword is the only thing that tells the two apart, and the
    // frontend used to drop it. Without it this transformation would
    // have added `Components::EventBase` to every valuetype it was
    // handed.
    let mut ast = model("valuetype V { public long x; };");
    let value = type_by_name(&ast, "V");
    assert!(
        transform_event_type(&mut ast, value).is_err(),
        "a valuetype was mapped as an eventtype"
    );
}

// ---------------------------------------------------------------------
// §6.7.2 — primary-key constraints.
// ---------------------------------------------------------------------

#[test]
fn a_primary_key_with_a_public_scalar_member_is_accepted() {
    let ast = model(
        "module Components { valuetype PrimaryKeyBase { public long b; }; };
         valuetype CKey : Components::PrimaryKeyBase { public long v; };",
    );
    let key = type_by_name(&ast, "CKey");
    assert_eq!(validate_primary_key(&ast, key), Ok(()));
}

#[test]
fn a_primary_key_without_the_base_is_rejected() {
    let ast = model("valuetype CKey { public long v; };");
    let key = type_by_name(&ast, "CKey");
    assert!(matches!(
        validate_primary_key(&ast, key),
        Err(PrimaryKeyError::NotDerivedFromPrimaryKeyBase(_))
    ));
}

#[test]
fn a_primary_key_with_a_private_member_is_rejected() {
    // §6.7.2 (p. 35). The check is only possible because the `private`
    // keyword now reaches the model.
    let ast = model(
        "module Components { valuetype PrimaryKeyBase { public long b; }; };
         valuetype CKey : Components::PrimaryKeyBase { private long v; };",
    );
    let key = type_by_name(&ast, "CKey");
    assert!(matches!(
        validate_primary_key(&ast, key),
        Err(PrimaryKeyError::HasPrivateStateMembers(_))
    ));
}

#[test]
fn a_primary_key_holding_an_object_reference_is_rejected() {
    let ast = model(
        "module Components { valuetype PrimaryKeyBase { public long b; }; };
         interface IFoo {};
         valuetype CKey : Components::PrimaryKeyBase { public IFoo v; };",
    );
    let key = type_by_name(&ast, "CKey");
    assert!(matches!(
        validate_primary_key(&ast, key),
        Err(PrimaryKeyError::HasInterfaceReference(_))
    ));
}

#[test]
fn a_primary_key_holding_a_nested_struct_is_accepted() {
    // The old check rejected every member written as a scoped name,
    // because a syntax tree could not tell a struct from an interface.
    // The model can, so §6.7.2 is now read as written rather than as the
    // conservative approximation the comment there admitted to.
    let ast = model(
        "module Components { valuetype PrimaryKeyBase { public long b; }; };
         struct Pair { long a; long b; };
         valuetype CKey : Components::PrimaryKeyBase { public Pair v; };",
    );
    let key = type_by_name(&ast, "CKey");
    assert_eq!(validate_primary_key(&ast, key), Ok(()));
}

#[test]
fn an_abstract_valuetype_is_not_a_primary_key() {
    let ast = model(
        "module Components { valuetype PrimaryKeyBase { public long b; }; };
         abstract valuetype CKey : Components::PrimaryKeyBase { };",
    );
    let key = type_by_name(&ast, "CKey");
    assert!(matches!(
        validate_primary_key(&ast, key),
        Err(PrimaryKeyError::NotValueType(_))
    ));
}

// ---------------------------------------------------------------------
// §6.7.3 — factory and finder body mapping.
// ---------------------------------------------------------------------

#[test]
fn a_factory_is_emitted_with_create_failure_first_in_its_raises() {
    let mut ast = model("component C {}; home CManager manages C {};");
    let home = interface_by_name(&ast, "CManager");
    let out = transform_home(&mut ast, home).expect("transform");
    let factory = InitOp::new("create_widget");
    zerodds_ccm::apply_factory_finder_body(&mut ast, &out, &[factory], &[]).expect("body");

    let id = ast
        .interface(out.explicit)
        .expect("explicit")
        .operations
        .iter()
        .copied()
        .find(|id| {
            ast.operation(*id)
                .is_ok_and(|node| node.name == "create_widget")
        })
        .expect("the factory operation");
    let raises = ast.operation(id).expect("factory").raises.clone();
    assert_eq!(raises.len(), 1);
    let zerodds_ast_core::Ref::Pending(zerodds_ast_core::PendingRef::ScopedName(name)) = &raises[0]
    else {
        panic!("the raises clause resolved to something");
    };
    assert_eq!(name.segments(), ["Components", "CreateFailure"]);
    // §6.7.3.1 — the factory returns the component the home manages.
    assert_eq!(return_name(&ast, out.explicit, "create_widget"), "C");
}

#[test]
fn a_finder_is_emitted_with_finder_failure_and_the_callers_own_raises() {
    let mut ast = model("component C {}; exception MyExcep {}; home CManager manages C {};");
    let home = interface_by_name(&ast, "CManager");
    let out = transform_home(&mut ast, home).expect("transform");
    let (excep, _) = ast
        .exceptions()
        .find(|(_, node)| node.name == "MyExcep")
        .expect("MyExcep");
    let mut finder = InitOp::new("find_by_size");
    finder.raises.push(zerodds_ast_core::Ref::Resolved(excep));
    zerodds_ccm::apply_factory_finder_body(&mut ast, &out, &[], &[finder]).expect("body");

    let id = ast
        .interface(out.explicit)
        .expect("explicit")
        .operations
        .iter()
        .copied()
        .find(|id| {
            ast.operation(*id)
                .is_ok_and(|node| node.name == "find_by_size")
        })
        .expect("the finder operation");
    let raises = ast.operation(id).expect("finder").raises.clone();
    assert_eq!(raises.len(), 2, "FinderFailure plus the caller's own");
    assert!(raises[1].is_resolved());
}

// ---------------------------------------------------------------------
// §13 — the lightweight profile.
// ---------------------------------------------------------------------

#[test]
fn the_lightweight_filter_drops_configurator_operations() {
    // §13.7 (p. 279).
    let mut ast = model("interface I {}; component C { provides I foo; };");
    let component = interface_by_name(&ast, "C");
    let out = transform_component(&mut ast, component).expect("transform");
    let _ = ast
        .add_operation(component, "configure", None, false)
        .expect("configure");
    filter_to_lightweight(&mut ast, &out).expect("filter");
    let names = op_names(&ast, component);
    assert!(names.contains(&"provide_foo".to_string()));
    assert!(!names.contains(&"configure".to_string()));
}

#[test]
fn the_lightweight_filter_keeps_the_type_specific_operations() {
    // §13.3 removes the generic navigation API, not the component's own
    // body.
    let mut ast =
        model("interface I {}; interface J {}; component C { provides I foo; uses J bar; };");
    let component = interface_by_name(&ast, "C");
    let out = transform_component(&mut ast, component).expect("transform");
    filter_to_lightweight(&mut ast, &out).expect("filter");
    let names = op_names(&ast, component);
    for expected in [
        "provide_foo",
        "connect_bar",
        "disconnect_bar",
        "get_connection_bar",
    ] {
        assert!(names.contains(&expected.to_string()), "missing {expected}");
    }
}

#[test]
fn the_lightweight_filter_reports_a_component_it_would_empty() {
    let mut ast = model("eventtype Tick {}; component C { consumes Tick sink; };");
    let component = interface_by_name(&ast, "C");
    let out = transform_component(&mut ast, component).expect("transform");
    // Remove the one operation the component had, leaving only the
    // configurator the caller then adds.
    let operations = ast.interface(component).expect("C").operations.clone();
    for id in operations {
        let node_id = ast.operation(id).expect("operation").node_id;
        ast.remove(zerodds_ast::AnyId::Operation(id, node_id))
            .expect("remove");
    }
    let _ = ast
        .add_operation(component, "configure", None, false)
        .expect("configure");
    assert_eq!(
        filter_to_lightweight(&mut ast, &out),
        Err(LightweightFilterError::EmptyAfterFilter)
    );
}

#[test]
fn a_component_equivalent_is_comparable() {
    let mut ast = model("component C {};");
    let component = interface_by_name(&ast, "C");
    let out = transform_component(&mut ast, component).expect("transform");
    assert_eq!(
        out,
        ComponentEquivalent {
            equivalent_interface: component,
            event_consumer_interfaces: Vec::new(),
        }
    );
}
