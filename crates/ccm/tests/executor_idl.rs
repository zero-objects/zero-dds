// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The executor equivalent IDL of CORBA 3.3 Part 3 §9.3, driven from IDL
//! through the real frontend.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used, missing_docs)]

use zerodds_ast::{Ast, InterfaceKind, InterfaceNode, ParamDirection, TypeKind};
use zerodds_ast_core::{Id, PendingRef, Ref};
use zerodds_ccm::{
    component_executors, executor_idl, home_executors, transform_component, transform_home,
};

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

fn interface(ast: &Ast, name: &str) -> Id<InterfaceNode> {
    ast.interfaces()
        .find(|(_, node)| node.name == name)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no interface named {name}"))
}

fn interface_ref_name(ast: &Ast, reference: &Ref<InterfaceNode>) -> String {
    match reference {
        Ref::Resolved(id) => ast
            .interface(*id)
            .map(|n| n.name.clone())
            .unwrap_or_default(),
        Ref::Pending(PendingRef::ScopedName(name)) => name.segments().join("::"),
        _ => String::new(),
    }
}

fn bases(ast: &Ast, id: Id<InterfaceNode>) -> Vec<String> {
    ast.interface(id)
        .expect("interface")
        .bases
        .iter()
        .map(|b| interface_ref_name(ast, b))
        .collect()
}

fn type_label(ast: &Ast, reference: &Ref<zerodds_ast::TypeNode>) -> String {
    match reference {
        Ref::Resolved(id) => {
            let node = ast.type_(*id).expect("type");
            match &node.kind {
                TypeKind::ObjectRef {
                    interface: Some(target),
                } => interface_ref_name(ast, target),
                TypeKind::String { .. } if node.name.is_empty() => "string".to_string(),
                _ => node.name.clone(),
            }
        }
        Ref::Pending(PendingRef::ScopedName(name)) => name.segments().join("::"),
        _ => String::new(),
    }
}

/// `name(params) -> return raises (…)` of every operation.
fn signatures(ast: &Ast, id: Id<InterfaceNode>) -> Vec<String> {
    ast.interface(id)
        .expect("interface")
        .operations
        .iter()
        .map(|op| {
            let node = ast.operation(*op).expect("operation");
            let params: Vec<String> = node
                .params
                .iter()
                .map(|p| {
                    let p = ast.param(*p).expect("param");
                    let dir = match p.direction {
                        ParamDirection::In => "in",
                        ParamDirection::Out => "out",
                        ParamDirection::InOut => "inout",
                    };
                    format!("{dir} {} {}", type_label(ast, &p.type_), p.name)
                })
                .collect();
            let returns = node
                .return_type
                .as_ref()
                .map_or_else(|| "void".to_string(), |r| type_label(ast, r));
            let raises: Vec<String> = node
                .raises
                .iter()
                .map(|r| match r {
                    Ref::Resolved(e) => ast
                        .exception(*e)
                        .map(|x| x.name.clone())
                        .unwrap_or_default(),
                    Ref::Pending(PendingRef::ScopedName(n)) => n.segments().join("::"),
                    _ => String::new(),
                })
                .collect();
            let mut s = format!("{returns} {}({})", node.name, params.join(", "));
            if !raises.is_empty() {
                s.push_str(&format!(" raises({})", raises.join(", ")));
            }
            s
        })
        .collect()
}

fn attributes(ast: &Ast, id: Id<InterfaceNode>) -> Vec<String> {
    ast.interface(id)
        .expect("interface")
        .attributes
        .iter()
        .map(|a| {
            let node = ast.attribute(*a).expect("attribute");
            format!(
                "{}{} {}",
                if node.readonly { "readonly " } else { "" },
                type_label(ast, &node.type_),
                node.name
            )
        })
        .collect()
}

/// §9.3.3.4: the example `interface Hello`, `component HelloWorld supports
/// Hello { attribute string message; }`.
#[test]
fn corba_p3_9_3_3_4_the_hello_world_example() {
    let mut ast = model(
        "interface Hello { void sayHello(); };
         component HelloWorld supports Hello { attribute string message; };",
    );
    let out = executor_idl(&mut ast).expect("executor IDL");
    let facet = interface(&ast, "CCM_Hello");
    assert_eq!(out.facets, [facet]);
    assert_eq!(ast.interface(facet).unwrap().kind, InterfaceKind::Local);
    assert_eq!(bases(&ast, facet), ["Hello"]);
    let c = &out.components[0];
    for (id, name) in [
        (c.main, "CCM_HelloWorld_Executor"),
        (c.monolithic, "CCM_HelloWorld"),
        (c.context, "CCM_HelloWorld_Context"),
    ] {
        let node = ast.interface(id).unwrap();
        assert_eq!(node.name, name);
        assert_eq!(node.kind, InterfaceKind::Local);
    }
    assert_eq!(
        bases(&ast, c.main),
        ["Components::EnterpriseComponent", "Hello"]
    );
    assert_eq!(
        bases(&ast, c.monolithic),
        ["Components::EnterpriseComponent", "Hello"]
    );
    assert_eq!(bases(&ast, c.context), ["Components::CCMContext"]);
    assert_eq!(attributes(&ast, c.main), ["string message"]);
    assert_eq!(attributes(&ast, c.monolithic), ["string message"]);
    assert!(attributes(&ast, c.context).is_empty());
    // The declarations themselves stay as they were.
    let component = interface(&ast, "HelloWorld");
    assert_eq!(
        ast.interface(component).unwrap().kind,
        InterfaceKind::Component
    );
}

/// §9.3.3.5 and §9.3.3.2: facets and sinks on the monolithic executor,
/// receptacles and sources on the context, a consumer executor per event
/// type.
#[test]
fn corba_p3_9_3_3_5_ports_become_operations_of_the_executor_and_the_context() {
    let mut ast = model(
        "interface Data {}; eventtype Tick {};
         component C {
             provides Data feed;
             uses Data peer;
             uses multiple Data peers;
             publishes Tick ticks;
             emits Tick alarm;
             consumes Tick sink;
         };",
    );
    let out = executor_idl(&mut ast).expect("executor IDL");
    let c = &out.components[0];
    assert_eq!(
        signatures(&ast, c.monolithic),
        ["CCM_Data get_feed()", "void push_sink(in Tick ev)"]
    );
    assert_eq!(
        signatures(&ast, c.context),
        [
            "Data get_connection_peer()",
            "C::peersConnections get_connections_peers()",
            "void push_ticks(in Tick ev)",
            "void push_alarm(in Tick ev)",
        ]
    );
    assert!(signatures(&ast, c.main).is_empty());
    let consumer = interface(&ast, "CCM_TickConsumer");
    assert_eq!(out.consumers, [consumer]);
    assert_eq!(signatures(&ast, consumer), ["void push(in Tick ev)"]);

    // The client-side equivalent IDL declares `C::peersConnections`, and
    // the context's operation resolves to it.
    let component = interface(&ast, "C");
    transform_component(&mut ast, component).expect("client side");
    let _ = ast.resolve_all_by_name();
    let get = ast
        .interface(c.context)
        .unwrap()
        .operations
        .iter()
        .copied()
        .find(|op| ast.operation(*op).unwrap().name == "get_connections_peers")
        .unwrap();
    let returns = ast.operation(get).unwrap().return_type.clone().unwrap();
    let Ref::Resolved(resolved) = returns else {
        panic!("not resolved: {returns:?}");
    };
    assert_eq!(ast.type_(resolved).unwrap().name, "peersConnections");
}

/// §9.3.3.3 rule 3 of each interface: a derived component's executors
/// inherit the base's.
#[test]
fn corba_p3_9_3_3_3_a_derived_component_inherits_the_executors_of_its_base() {
    let mut ast = model(
        "component B { attribute long x; };
         component D : B { attribute long y; };",
    );
    let derived = interface(&ast, "D");
    let d = component_executors(&mut ast, derived).expect("D");
    assert_eq!(bases(&ast, d.main), ["CCM_B_Executor"]);
    assert_eq!(bases(&ast, d.monolithic), ["CCM_B"]);
    assert_eq!(bases(&ast, d.context), ["CCM_B_Context"]);
    assert_eq!(attributes(&ast, d.main), ["long y"]);
}

/// An extended port is its basic ports (§7.2.2.2) before §9.3.3.5 maps
/// them.
#[test]
fn corba_p3_9_3_3_5_an_extended_port_maps_through_its_basic_ports() {
    let mut ast = model(
        "interface Data {};
         porttype P { provides Data d; uses Data u; };
         component C { port P p; mirrorport P m; };",
    );
    let component = interface(&ast, "C");
    let c = component_executors(&mut ast, component).expect("C");
    assert_eq!(
        signatures(&ast, c.monolithic),
        ["CCM_Data get_p_d()", "CCM_Data get_m_u()"]
    );
    assert_eq!(
        signatures(&ast, c.context),
        ["Data get_connection_p_u()", "Data get_connection_m_d()"]
    );
}

/// §9.3.3.6.9: `home Bank manages Account { factory open(in string name);
/// void close(in string name); }`.
#[test]
fn corba_p3_9_3_3_6_9_the_bank_example() {
    let mut ast = model(
        "component Account {};
         home Bank manages Account {
             factory open(in string name);
             void close(in string name);
             attribute long branches;
         };",
    );
    let home = interface(&ast, "Bank");
    let h = home_executors(&mut ast, home).expect("Bank");
    assert_eq!(ast.interface(h.explicit).unwrap().name, "CCM_BankExplicit");
    assert_eq!(bases(&ast, h.explicit), ["Components::HomeExecutorBase"]);
    assert_eq!(
        signatures(&ast, h.explicit),
        [
            "Components::EnterpriseComponent open(in string name)",
            "void close(in string name)",
        ]
    );
    assert_eq!(attributes(&ast, h.explicit), ["long branches"]);
    assert!(bases(&ast, h.implicit).is_empty());
    assert_eq!(
        signatures(&ast, h.implicit),
        ["Components::EnterpriseComponent create() raises(Components::CCMException)"]
    );
    assert_eq!(
        bases(&ast, h.main),
        ["CCM_BankExplicit", "CCM_BankImplicit"]
    );
    assert!(signatures(&ast, h.main).is_empty());
    for id in [h.explicit, h.implicit, h.main] {
        assert_eq!(ast.interface(id).unwrap().kind, InterfaceKind::Local);
    }
    // The client-side transformation still works on the home afterwards.
    transform_home(&mut ast, home).expect("client side");
}

/// §9.3.3.6.4: a keyed home, and a keyless home derived from it, have
/// `create`, `find_by_primary_key` and `remove` with the key; a finder
/// returns `EnterpriseComponent` (§9.3.3.6.7); the derived home's explicit
/// executor inherits the base's.
#[test]
fn corba_p3_9_3_3_6_4_a_keyed_home_and_a_keyless_home_derived_from_it() {
    let mut ast = model(
        "valuetype Key { public long k; };
         component Account {};
         home Bank manages Account primarykey Key { finder by_name(in string n); };
         home Branch : Bank manages Account {};",
    );
    let bank = interface(&ast, "Bank");
    let branch = interface(&ast, "Branch");
    let b = home_executors(&mut ast, bank).expect("Bank");
    let keyed = [
        "Components::EnterpriseComponent create(in Key key) raises(Components::CCMException)",
        "Components::EnterpriseComponent find_by_primary_key(in Key key) raises(Components::CCMException)",
        "void remove(in Key key) raises(Components::CCMException)",
    ];
    assert_eq!(signatures(&ast, b.implicit), keyed);
    assert_eq!(
        signatures(&ast, b.explicit),
        ["Components::EnterpriseComponent by_name(in string n)"]
    );
    let d = home_executors(&mut ast, branch).expect("Branch");
    assert_eq!(signatures(&ast, d.implicit), keyed);
    assert_eq!(bases(&ast, d.explicit), ["CCM_BankExplicit"]);
}

/// What is no component or home is refused.
#[test]
fn corba_p3_9_3_the_executor_transformations_refuse_other_declarations() {
    let mut ast = model("interface I {}; local interface L {}; component C {};");
    let i = interface(&ast, "I");
    let l = interface(&ast, "L");
    let c = interface(&ast, "C");
    assert!(component_executors(&mut ast, i).is_err());
    assert!(home_executors(&mut ast, c).is_err());
    assert!(zerodds_ccm::facet_executor(&mut ast, l).is_err());
    assert!(zerodds_ccm::facet_executor(&mut ast, c).is_err());
}
