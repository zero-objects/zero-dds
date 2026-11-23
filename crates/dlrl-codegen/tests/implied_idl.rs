// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS 1.2 §8.2.1.2.2: the implied IDL that `generate_implied_idl` writes
//! for application classes, compiled by the IDL frontend after the
//! generic IDL of `crates/dlrl/idl/dds_dlrl.idl` and the classes'
//! valuetypes (§8.2.2.2).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_ast::{AnyId, Ast, InterfaceKind, TypeKind};
use zerodds_ast_core::ScopedName;
use zerodds_dlrl_codegen::generate_implied_idl;
use zerodds_idl_compose::{PathLoader, compose_source};

const DLRL_IDL: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../dlrl/idl");

/// The model description of §8.2.3.2 (Track, Track3D, Radar), as
/// valuetypes deriving from DDS::ObjectRoot, in module demo, and a class
/// Foo at global scope.
const MODEL: &str = "module demo {
    valuetype Radar : DDS::ObjectRoot {
        public string name;
    };
    valuetype Track : DDS::ObjectRoot {
        public double x;
        public double y;
    };
    valuetype Track3D : Track {
        public double z;
    };
};
valuetype Foo : DDS::ObjectRoot {
    public long n;
};
";

fn compose(source: &str) -> (Ast, Vec<String>) {
    let composed = compose_source(
        &format!("{DLRL_IDL}/implied_test.idl"),
        source,
        &PathLoader::bare(),
    );
    let diagnostics = composed
        .diagnostics
        .iter()
        .map(|d| format!("{d:?}"))
        .collect();
    (composed.ast, diagnostics)
}

fn path(name: &str) -> ScopedName {
    ScopedName::absolute(name.split("::").map(ToString::to_string).collect())
}

fn interface_bases(ast: &Ast, name: &str) -> (InterfaceKind, Vec<String>, Vec<String>) {
    let Ok(AnyId::Interface(id, _)) = ast.find_by_scoped_name(&path(name)) else {
        panic!("{name} is not an interface");
    };
    let node = ast.interface(id).unwrap();
    let bases = node
        .bases
        .iter()
        .map(|b| {
            let id = b.resolved().expect("a base resolves");
            let n = ast.interface(id).unwrap();
            ast.scoped_name_of(AnyId::Interface(id, n.node_id))
                .unwrap()
                .segments()
                .join("::")
        })
        .collect();
    let ops = node
        .operations
        .iter()
        .map(|o| ast.operation(*o).unwrap().name.clone())
        .collect();
    (node.kind, bases, ops)
}

fn valuetype_base(ast: &Ast, name: &str) -> String {
    let Ok(AnyId::Type(id, _)) = ast.find_by_scoped_name(&path(name)) else {
        panic!("{name} is not a type");
    };
    let TypeKind::Valuetype { bases, .. } = &ast.type_(id).unwrap().kind else {
        panic!("{name} is not a valuetype");
    };
    let base = bases[0].resolved().unwrap();
    let node = ast.type_(base).unwrap();
    ast.scoped_name_of(AnyId::Type(base, node.node_id))
        .unwrap()
        .segments()
        .join("::")
}

/// §8.2.1.2.2: the implied IDL of Track, Track3D, Radar and Foo compiles
/// after the generic IDL, and each derived construct has its base.
#[test]
fn dlrl_8_2_1_2_implied_idl_for_track_track3d_radar_parses() {
    let mut source = format!("#include \"dds_dlrl.idl\"\n{MODEL}");
    for class in ["demo::Radar", "demo::Track", "demo::Track3D", "Foo"] {
        source.push_str(&generate_implied_idl(class));
    }
    let (ast, diagnostics) = compose(&source);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");

    let (kind, bases, ops) = interface_bases(&ast, "demo::TrackHome");
    assert_eq!(kind, InterfaceKind::Local);
    assert_eq!(bases, ["DDS::ObjectHome"]);
    assert_eq!(
        ops,
        [
            "attach_listener",
            "detach_listener",
            "create_selection",
            "delete_selection",
            "create_object",
            "create_unregistered_object",
            "register_object",
            "find_object_in_access",
            "find_object",
            "get_objects",
            "get_created_objects",
            "get_modified_objects",
            "get_deleted_objects",
        ]
    );
    let (_, bases, _) = interface_bases(&ast, "demo::TrackQuery");
    assert_eq!(bases, ["DDS::QueryCriterion", "demo::TrackFilter"]);
    let (_, bases, ops) = interface_bases(&ast, "demo::Track3DListener");
    assert_eq!(bases, ["DDS::ObjectListener"]);
    assert_eq!(
        ops,
        [
            "on_object_created",
            "on_object_modified",
            "on_object_deleted"
        ]
    );
    let (_, bases, _) = interface_bases(&ast, "demo::RadarSelectionListener");
    assert_eq!(bases, ["DDS::SelectionListener"]);
    let (_, bases, _) = interface_bases(&ast, "FooSelection");
    assert_eq!(bases, ["DDS::Selection"]);
    for (derived, base) in [
        ("demo::TrackList", "DDS::List"),
        ("demo::TrackSet", "DDS::Set"),
        ("demo::TrackStrMap", "DDS::StrMap"),
        ("demo::TrackIntMap", "DDS::IntMap"),
        ("FooIntMap", "DDS::IntMap"),
    ] {
        assert_eq!(valuetype_base(&ast, derived), base, "{derived}");
    }
    for alias in [
        "demo::TrackSeq",
        "demo::TrackListenerSeq",
        "demo::TrackSelectionSeq",
        "FooSeq",
    ] {
        assert!(
            matches!(ast.find_by_scoped_name(&path(alias)), Ok(AnyId::Type(..))),
            "{alias}"
        );
    }
}

/// Why the generic IDL of this tree has the listeners' typed operations
/// only in comments: with `on_object_created` declared in
/// `DDS::ObjectListener`, as the printed generic IDL does, the implied
/// `FooListener` redefines it, which IDL forbids (IDL 4.2 §7.4.3), and the
/// frontend refuses the file. The refusal names the redefinition of
/// `on_object_created`, and the same file without the derived
/// declaration has no diagnostic: the refusal comes from the
/// redefinition alone.
#[test]
fn dlrl_8_2_1_2_the_printed_listener_redefinition_is_refused() {
    let file = |derived: &str| {
        format!(
            "#include \"dds_dcps.idl\"
module DDS {{
    valuetype ObjectRoot {{ public long n; }};
    local interface ObjectListener {{
        boolean on_object_created(in ObjectRoot the_object);
    }};
}};
valuetype Foo : DDS::ObjectRoot {{ public long m; }};
local interface FooListener : DDS::ObjectListener {{
{derived}}};
"
        )
    };
    let (_, diagnostics) = compose(&file("    boolean on_object_created(in Foo the_object);\n"));
    assert!(
        diagnostics.iter().any(
            |d| d.contains("InvalidInheritance") && d.contains("`on_object_created` redefines")
        ),
        "no diagnostic of the redefinition of on_object_created: {diagnostics:?}"
    );
    let (_, diagnostics) = compose(&file(""));
    assert!(
        diagnostics.is_empty(),
        "the file without the redefinition: {diagnostics:?}"
    );
}
