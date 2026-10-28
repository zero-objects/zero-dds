// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The constructs outside plain data types: the component model,
//! template modules, repository identifiers and vendor directives.
//!
//! Each one keeps what was written rather than being normalised into
//! something the core already has. A port turned into a plain attribute
//! and a `keylist` turned into `@key` both read back as different source
//! than went in. Spec §4.2, §4.6 of the AST.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::{
    AnyId, Ast, ComponentRole, InterfaceKind, RepositoryIdOrigin, TemplateParamKind,
};
use zerodds_ast_core::ScopedName;
use zerodds_idl_lower::lower;
use zerodds_idl_parse::parse;

fn model(source: &str) -> Ast {
    let parsed = parse(source);
    assert!(
        !parsed.has_errors(),
        "the fixture does not parse: {:?}",
        parsed.diagnostics()
    );
    lower(&parsed.syntax(), "test.idl").ast
}

fn scoped(segments: &[&str]) -> ScopedName {
    ScopedName::absolute(segments.iter().map(|s| (*s).to_string()).collect())
}

fn interface_of(ast: &Ast, name: &str) -> zerodds_ast_core::Id<zerodds_ast::InterfaceNode> {
    match ast.find_by_scoped_name(&scoped(&[name])) {
        Ok(AnyId::Interface(id, _)) => id,
        other => panic!("{name} is not an interface: {other:?}"),
    }
}

/// The component role of a named port.
fn role_of(ast: &Ast, interface: &str, port: &str) -> ComponentRole {
    let id = interface_of(ast, interface);
    for attribute in ast.attributes_of(id) {
        let node = ast.attribute(*attribute).expect("attribute");
        if node.name != port {
            continue;
        }
        return ast
            .facets(AnyId::Attribute(*attribute, node.node_id))
            .expect("facets")
            .corba
            .as_ref()
            .and_then(|c| c.component_role.clone())
            .unwrap_or_else(|| panic!("{port} carries no component role"));
    }
    panic!("{interface} has no port named {port}");
}

// -- the component model -------------------------------------------------

#[test]
fn each_component_form_reaches_its_own_interface_kind() {
    for (source, name, expected) in [
        ("component C { };", "C", InterfaceKind::Component),
        ("home H manages C { };", "H", InterfaceKind::Home),
        ("porttype P { };", "P", InterfaceKind::PortType),
        ("connector K { };", "K", InterfaceKind::Connector),
        ("interface I { };", "I", InterfaceKind::Interface),
        ("local interface L { };", "L", InterfaceKind::Local),
        ("abstract interface A { };", "A", InterfaceKind::Abstract),
    ] {
        let ast = model(source);
        let id = interface_of(&ast, name);
        assert_eq!(
            ast.interface(id).expect("interface").kind,
            expected,
            "{source}"
        );
    }
}

#[test]
fn each_port_form_reaches_its_own_component_role() {
    let ast = model(
        "interface IService { void go(); };
         interface ISource { void pull(); };
         eventtype Reading { public long value; };
         component C {
           provides IService srv;
           uses ISource src;
           uses multiple ISource many;
           emits Reading emitted;
           publishes Reading published;
           consumes Reading consumed;
         };",
    );

    assert!(matches!(
        role_of(&ast, "C", "srv"),
        ComponentRole::Facet { .. }
    ));
    match role_of(&ast, "C", "src") {
        ComponentRole::Receptacle { multiple, .. } => assert!(!multiple),
        other => panic!("expected a receptacle, got {other:?}"),
    }
    match role_of(&ast, "C", "many") {
        ComponentRole::Receptacle { multiple, .. } => {
            assert!(multiple, "`uses multiple` lost its multiplicity");
        }
        other => panic!("expected a receptacle, got {other:?}"),
    }
    match role_of(&ast, "C", "emitted") {
        ComponentRole::EventSource { publisher, .. } => {
            assert!(!publisher, "`emits` was recorded as `publishes`");
        }
        other => panic!("expected an event source, got {other:?}"),
    }
    match role_of(&ast, "C", "published") {
        ComponentRole::EventSource { publisher, .. } => {
            assert!(publisher, "`publishes` was recorded as `emits`");
        }
        other => panic!("expected an event source, got {other:?}"),
    }
    assert!(matches!(
        role_of(&ast, "C", "consumed"),
        ComponentRole::EventSink { .. }
    ));
}

#[test]
fn a_home_records_what_it_manages_and_the_key_it_does_so_by() {
    let ast = model(
        "component C { };
         struct K { long id; };
         home H manages C primarykey K { };",
    );
    let id = interface_of(&ast, "H");
    let node_id = ast.interface(id).expect("H").node_id;
    let manages = ast
        .facets(AnyId::Interface(id, node_id))
        .expect("facets")
        .corba
        .as_ref()
        .and_then(|c| c.manages.clone())
        .expect("a manages clause");
    assert!(manages.component.is_resolved(), "the component was lost");
    assert!(
        manages.primary_key.is_some_and(|key| key.is_resolved()),
        "the primary key was lost"
    );
}

// -- template modules ----------------------------------------------------

#[test]
fn a_template_modules_parameters_reach_its_facet() {
    let ast = model("module Buffer<typename T, const long Capacity> { };");
    let module = ast
        .module_by_path(&scoped(&["Buffer"]))
        .expect("the module");
    let node_id = ast.module(module).expect("module").node_id;
    let params = ast
        .facets(AnyId::Module(module, node_id))
        .expect("facets")
        .corba
        .as_ref()
        .map(|c| c.template_params.clone())
        .unwrap_or_default();

    assert_eq!(params.len(), 2);
    assert_eq!(params[0].name, "T");
    assert_eq!(params[0].kind, TemplateParamKind::TypeName);
    assert_eq!(params[1].name, "Capacity");
    assert_eq!(params[1].kind, TemplateParamKind::Const);
    assert!(
        params[1].constraint.is_some(),
        "a const parameter without its type"
    );
}

#[test]
fn each_template_parameter_kind_is_distinguished() {
    for (written, expected) in [
        ("typename T", TemplateParamKind::TypeName),
        ("struct T", TemplateParamKind::Struct),
        ("union T", TemplateParamKind::Union),
        ("enum T", TemplateParamKind::Enum),
        ("exception T", TemplateParamKind::Exception),
        ("interface T", TemplateParamKind::Interface),
        ("valuetype T", TemplateParamKind::ValueType),
        ("eventtype T", TemplateParamKind::EventType),
        ("sequence T", TemplateParamKind::Sequence),
    ] {
        let source = format!("module M<{written}> {{ }};");
        let ast = model(&source);
        let module = ast.module_by_path(&scoped(&["M"])).expect("the module");
        let node_id = ast.module(module).expect("module").node_id;
        let params = ast
            .facets(AnyId::Module(module, node_id))
            .expect("facets")
            .corba
            .as_ref()
            .map(|c| c.template_params.clone())
            .unwrap_or_default();
        assert_eq!(params.first().map(|p| p.kind), Some(expected), "{written}");
    }
}

// -- repository identifiers ----------------------------------------------

#[test]
fn a_type_gets_a_repository_id_derived_from_its_scoped_name() {
    let ast = model("module Robot { struct Pose { long x; }; };");
    let any = ast
        .find_by_scoped_name(&scoped(&["Robot", "Pose"]))
        .expect("Pose");
    let corba = ast
        .facets(any)
        .expect("facets")
        .corba
        .as_ref()
        .expect("a corba facet");
    assert_eq!(
        corba.repository_id.as_ref().map(|id| id.0.as_str()),
        Some("IDL:Robot/Pose:1.0")
    );
    assert_eq!(
        corba.repository_id_origin,
        Some(RepositoryIdOrigin::Derived)
    );
}

/// A declaration inside an interface is named through it. CosNaming's
/// `exception NotFound` sits in `interface NamingContext`, and omniORB,
/// TAO and JacORB raise and expect `IDL:omg.org/CosNaming/NamingContext/
/// NotFound:1.0`; with the interface left out, a ZeroDDS naming server
/// raised an id none of them knew and a ZeroDDS client did not recognise
/// theirs (measured with `crates/corba-interop/competitors/run_interop.sh`).
#[test]
fn a_declaration_in_an_interface_is_named_through_it() {
    let ast = model(
        "#pragma prefix \"omg.org\"
         module CosNaming {
           interface NamingContext {
             exception NotFound { long why; };
             typedef long Count;
             const long LIMIT = 3;
             void unbind() raises (NotFound);
           };
         };",
    );
    for (name, id) in [
        (
            "NotFound",
            "IDL:omg.org/CosNaming/NamingContext/NotFound:1.0",
        ),
        ("Count", "IDL:omg.org/CosNaming/NamingContext/Count:1.0"),
    ] {
        let any = ast
            .find_by_scoped_name(&scoped(&["CosNaming", "NamingContext", name]))
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let corba = ast
            .facets(any)
            .expect("facets")
            .corba
            .as_ref()
            .expect("a corba facet");
        assert_eq!(
            corba.repository_id.as_ref().map(|id| id.0.as_str()),
            Some(id),
            "{name}"
        );
    }
    assert!(
        ast.find_by_scoped_name(&scoped(&["CosNaming", "NamingContext", "LIMIT"]))
            .is_ok(),
        "the constant"
    );
    assert!(
        ast.find_by_scoped_name(&scoped(&["CosNaming", "NotFound"]))
            .is_err(),
        "the name without the interface names nothing"
    );
}

#[test]
fn a_typeid_directive_sets_the_id_outright() {
    let ast = model(
        "module Robot { struct Pose { long x; }; };
         typeid Robot::Pose \"IDL:custom/Pose:2.0\";",
    );
    let any = ast
        .find_by_scoped_name(&scoped(&["Robot", "Pose"]))
        .expect("Pose");
    let corba = ast
        .facets(any)
        .expect("facets")
        .corba
        .as_ref()
        .expect("a corba facet");
    assert_eq!(
        corba.repository_id.as_ref().map(|id| id.0.as_str()),
        Some("IDL:custom/Pose:2.0")
    );
    assert_eq!(
        corba.repository_id_origin,
        Some(RepositoryIdOrigin::PragmaId),
        "an id set outright still claimed to be derived"
    );
}

#[test]
fn a_typeprefix_directive_prefixes_the_ids_derived_below_it() {
    let ast = model(
        "module Robot { struct Pose { long x; }; };
         typeprefix Robot \"example.com\";",
    );
    let any = ast
        .find_by_scoped_name(&scoped(&["Robot", "Pose"]))
        .expect("Pose");
    let corba = ast
        .facets(any)
        .expect("facets")
        .corba
        .as_ref()
        .expect("a corba facet");
    assert_eq!(
        corba.repository_id.as_ref().map(|id| id.0.as_str()),
        Some("IDL:example.com/Robot/Pose:1.0")
    );
    assert_eq!(
        corba.repository_id_origin,
        Some(RepositoryIdOrigin::PragmaPrefix)
    );
}

#[test]
fn an_import_is_kept_as_written() {
    let ast = model("import \"IDL:Other:1.0\"; struct S { long x; };");
    let root = ast.root_module();
    let node_id = ast.module(root).expect("root").node_id;
    let imports = ast
        .facets(AnyId::Module(root, node_id))
        .expect("facets")
        .corba
        .as_ref()
        .map(|c| c.imports.clone())
        .unwrap_or_default();
    assert_eq!(imports, vec!["IDL:Other:1.0".to_string()]);
}

// -- vendor directives ---------------------------------------------------

/// `keylist T (a)` reaches **both** the vendor facet and the member.
///
/// This test used to assert the opposite for the member half, on the
/// reasoning that `keylist T (a)` and `@key long a;` say the same thing
/// in two dialects and turning one into the other loses which was
/// written. The first half of that is right and is why the facet is
/// still set. The second half made the directive mean nothing:
/// `Facets::is_key` answers from `xtypes.is_key` alone, and the only
/// other reader of `vendor.rti.keylist` in this tree is the snapshot
/// serialiser. An RTI or Cyclone `.idl` therefore produced a type with
/// **no key** — every sample in one instance, a constant KeyHash, and
/// nothing reporting it.
///
/// Keeping the facet keeps the round trip back to that dialect. Setting
/// the member is what makes the file mean what it says.
#[test]
fn an_rti_keylist_reaches_the_vendor_facet_and_the_members() {
    let ast = model(
        "module rti_demo {
           struct Reading { long sensor_id; double value; };
           keylist Reading (sensor_id);
         };",
    );
    let any = ast
        .find_by_scoped_name(&scoped(&["rti_demo", "Reading"]))
        .expect("Reading");
    let rti = ast
        .facets(any)
        .expect("facets")
        .vendor
        .rti
        .as_ref()
        .expect("an RTI facet");
    assert_eq!(rti.keylist, vec!["sensor_id".to_string()]);

    // And the member the directive names is a key.
    let AnyId::Type(id, _) = any else {
        panic!("expected a type");
    };
    let keyed = |name: &str| {
        let member = ast
            .members_of(id)
            .iter()
            .copied()
            .find(|m| ast.member(*m).expect("member").name == name)
            .expect("the member");
        let node = ast.member(member).expect("member");
        ast.facets(AnyId::Member(member, node.node_id))
            .expect("facets")
            .xtypes
            .as_ref()
            .is_some_and(|x| x.is_key)
    };
    assert!(keyed("sensor_id"), "the directive names it");
    assert!(!keyed("value"), "and does not name this one");
}

#[test]
fn a_keylist_naming_a_type_that_is_not_there_is_reported() {
    let parsed = parse("keylist Missing (a);");
    let lowered = lower(&parsed.syntax(), "test.idl");
    assert!(
        lowered
            .diagnostics
            .iter()
            .any(|d| d.code.as_str() == "IDL0010"),
        "{:?}",
        lowered.diagnostics
    );
}

// -- --strict, per vendor extension --------------------------------------

/// Lower `source` under a dialect and hand back the diagnostics.
fn diagnostics_under(source: &str, dialect: zerodds_idl_parse::Dialect) -> Vec<String> {
    let parsed = zerodds_idl_parse::parse_with(source, dialect);
    let lowered = zerodds_idl_lower::lower_with(
        &parsed.syntax(),
        "test.idl",
        zerodds_idl_lower::Options {
            dialect,
            ..zerodds_idl_lower::Options::default()
        },
    );
    parsed
        .diagnostics()
        .iter()
        .chain(lowered.diagnostics.iter())
        .filter(|d| d.severity == zerodds_idl_parse::Severity::Error)
        .map(|d| format!("{} {}", d.code.as_str(), d.message))
        .collect()
}

/// One source per vendor extension the front end acts on, and the same
/// source read twice: under `STRICT`, where it must be an error, and
/// under `ALL_VENDORS`, where it must not be.
///
/// The pairing is the point. A rule that rejects under both dialects
/// would pass a one-sided assertion and break every file in the tree
/// that uses one of these directives; `--strict` is a question about
/// portability, not a ban.
#[test]
fn strict_rejects_every_vendor_extension_the_front_end_acts_on() {
    let cases: [(&str, &str); 4] = [
        // RTI Connext: `keylist` as a top-level production.
        ("rti keylist", "struct Sa { long m; };\nkeylist Sa (m);\n"),
        // Cyclone DDS: the same thing said as a pragma.
        (
            "cyclone #pragma keylist",
            "#pragma keylist Sa m\nstruct Sa { long m; };\n",
        ),
        // OpenDDS: the topic-type directive.
        (
            "opendds #pragma DCPS_DATA_TYPE",
            "#pragma DCPS_DATA_TYPE \"Sa\"\nstruct Sa { long m; };\n",
        ),
        // OpenDDS: and the key directive.
        (
            "opendds #pragma DCPS_DATA_KEY",
            "#pragma DCPS_DATA_TYPE \"Sa\"\n#pragma DCPS_DATA_KEY Sa m\n\
             struct Sa { long m; };\n",
        ),
    ];

    for (what, source) in cases {
        let strict = diagnostics_under(source, zerodds_idl_parse::Dialect::STRICT);
        assert!(
            strict.iter().any(|d| d.starts_with("IDL0005")),
            "{what}: --strict must reject it, got {strict:?}"
        );
        let lenient = diagnostics_under(source, zerodds_idl_parse::Dialect::ALL_VENDORS);
        assert!(
            lenient.is_empty(),
            "{what}: without --strict the same file must get through, got {lenient:?}"
        );
    }
}

/// An unknown annotation is not a vendor extension `--strict` may
/// refuse. IDL 4.2 §7.4.15.4.2: "in case an unknown annotation is
/// encountered, it should be ignored by the IDL-processing tools."
///
/// `@RTI_*` and `@RTPS_*` are exactly that — the front end does not act
/// on any of them, so refusing them under `--strict` would break
/// conforming IDL to gate nothing. (`@Key` is `@key`: DDS-XTypes 1.3
/// §7.3.1.2.5 makes annotation names case-insensitive.)
#[test]
fn strict_does_not_refuse_an_unknown_annotation() {
    for source in [
        "struct Sa { @Key long m; };\n",
        "@RTI_topic_kind(NO_KEY) struct Sa { long m; };\n",
        "@RTPS_data_representation(XCDR2) struct Sa { long m; };\n",
        "@nosuchthing struct Sa { long m; };\n",
    ] {
        let strict = diagnostics_under(source, zerodds_idl_parse::Dialect::STRICT);
        assert!(
            strict.is_empty(),
            "{source:?}: §7.4.15.4.2 says ignore it, got {strict:?}"
        );
    }
}

// -- valuetypes ----------------------------------------------------------

#[test]
fn a_valuetype_keeps_its_modifier_bases_and_supported_interfaces() {
    use zerodds_ast::{TypeKind, ValuetypeKind};

    let ast = model(
        "interface IFace { void op(); };
         valuetype Base { public long base_field; };
         custom valuetype V : truncatable Base supports IFace {
           public long counter;
           private string secret;
         };",
    );
    let AnyId::Type(id, _) = ast.find_by_scoped_name(&scoped(&["V"])).expect("V") else {
        panic!("expected a type");
    };
    let TypeKind::Valuetype {
        kind,
        bases,
        state,
        supported,
        ..
    } = &ast.type_(id).expect("V").kind
    else {
        panic!("expected a valuetype");
    };
    // `custom` and `truncatable` are both written; the last one read
    // decides, and both are modifiers of the same declaration.
    assert!(matches!(
        kind,
        ValuetypeKind::Custom | ValuetypeKind::Truncatable
    ));
    assert_eq!(bases.len(), 1, "the base valuetype was lost");
    assert_eq!(supported.len(), 1, "the supported interface was lost");
    assert_eq!(state.len(), 2, "a state member was lost");
}

#[test]
fn a_valuetype_initialiser_becomes_an_operation_of_it() {
    let ast = model(
        "exception Bad { long code; };
         valuetype V {
           public long counter;
           factory create(in long initial) raises (Bad);
           long get_counter();
         };",
    );
    let AnyId::Type(id, _) = ast.find_by_scoped_name(&scoped(&["V"])).expect("V") else {
        panic!("expected a type");
    };
    let zerodds_ast::TypeKind::Valuetype { state, .. } = &ast.type_(id).expect("V").kind else {
        panic!("expected a valuetype");
    };
    assert_eq!(state.len(), 1, "the operation was counted as state");
}

#[test]
fn a_declaration_exported_from_a_valuetype_body_is_lowered() {
    // Rule (105)'s `<export>` puts a nested `exception`, `const`,
    // `typedef` or type declaration in a valuetype body beside its
    // state. The body loop took `Member` and `Operation` and nothing
    // else, so every one of them was parsed, kept in the CST and dropped
    // with no diagnostic —
    // `crates/idl-parse/tests/fixtures/spec_features/corba_value_repository.idl`
    // declares `exception Bad {};` inside `valuetype V` and names it in
    // `factory create(..) raises (Bad)`, so the `raises` reference had
    // nothing to resolve to.
    //
    // The exported names are named through the valuetype, as an
    // interface's exports are through the interface: `V::Bad`, and its
    // repository id `IDL:V/Bad:1.0`. They used to land in the enclosing
    // module, and CosNaming's `NamingContext::NotFound` with them, which
    // omniORB, TAO and JacORB do not know.
    let ast = model(
        "valuetype V {
           public long counter;
           exception Bad { long code; };
           const long LIMIT = 7;
           typedef sequence<long> Longs;
           struct Inner { long y; };
           factory create(in long initial) raises (Bad);
         };",
    );
    for name in ["Bad", "LIMIT", "Longs", "Inner"] {
        assert!(
            ast.find_by_scoped_name(&scoped(&["V", name])).is_ok(),
            "`{name}` was declared in the valuetype body and is not in the model as `V::{name}`"
        );
    }
    // And the `raises` clause of the initialiser reaches the exception
    // the body declared, rather than a name with no declaration.
    let AnyId::Type(id, _) = ast.find_by_scoped_name(&scoped(&["V"])).expect("V") else {
        panic!("expected a type");
    };
    let zerodds_ast::TypeKind::Valuetype { initializers, .. } = &ast.type_(id).expect("V").kind
    else {
        panic!("expected a valuetype");
    };
    let create = initializers.first().expect("the initialiser");
    let raises = &ast.operation(*create).expect("create").raises;
    assert_eq!(raises.len(), 1, "the `raises` clause was lost");
    assert!(
        raises[0].resolved().is_some(),
        "`raises (Bad)` did not resolve to the exception the body declares"
    );
}

#[test]
fn an_attribute_in_a_valuetype_body_is_reported_rather_than_dropped() {
    // `AttributeNode::parent` is an `Id<InterfaceNode>`, so the model
    // has no place for an attribute declared in a valuetype body. A gap
    // that reports is a gap; a gap that is silent is an acceptance
    // claim.
    let parsed = parse(
        "valuetype V {
           public long counter;
           attribute long a;
         };",
    );
    let lowered = lower(&parsed.syntax(), "test.idl");
    assert!(
        lowered
            .diagnostics
            .iter()
            .any(|d| d.message.contains("valuetype body")),
        "the attribute was dropped without a diagnostic: {:?}",
        lowered.diagnostics
    );
}

// -- pragmas -------------------------------------------------------------

#[test]
fn a_pragma_id_sets_the_repository_id_like_typeid_does() {
    // The two spellings say one thing; both reach the same field with
    // the same origin. Spec §3.3 of the frontend.
    let ast = model("struct Pose { long x; };\n#pragma ID Pose \"IDL:custom/Pose:3.0\"\n");
    let any = ast.find_by_scoped_name(&scoped(&["Pose"])).expect("Pose");
    let corba = ast
        .facets(any)
        .expect("facets")
        .corba
        .as_ref()
        .expect("a corba facet");
    assert_eq!(
        corba.repository_id.as_ref().map(|id| id.0.as_str()),
        Some("IDL:custom/Pose:3.0")
    );
    assert_eq!(
        corba.repository_id_origin,
        Some(RepositoryIdOrigin::PragmaId)
    );
}

#[test]
fn a_pragma_version_changes_the_version_a_derived_id_ends_in() {
    let ast = model("struct Pose { long x; };\n#pragma version Pose 2.1\n");
    let any = ast.find_by_scoped_name(&scoped(&["Pose"])).expect("Pose");
    let corba = ast
        .facets(any)
        .expect("facets")
        .corba
        .as_ref()
        .expect("a corba facet");
    assert_eq!(
        corba.repository_id.as_ref().map(|id| id.0.as_str()),
        Some("IDL:Pose:2.1")
    );
    assert_eq!(
        corba.repository_id_origin,
        Some(RepositoryIdOrigin::PragmaVersion)
    );
}

#[test]
fn a_pragma_prefix_prefixes_the_ids_of_its_module() {
    let ast = model("module Robot {\n#pragma prefix \"example.com\"\nstruct Pose { long x; };\n};");
    let any = ast
        .find_by_scoped_name(&scoped(&["Robot", "Pose"]))
        .expect("Pose");
    let corba = ast
        .facets(any)
        .expect("facets")
        .corba
        .as_ref()
        .expect("a corba facet");
    assert_eq!(
        corba.repository_id.as_ref().map(|id| id.0.as_str()),
        Some("IDL:example.com/Robot/Pose:1.0")
    );
}

#[test]
fn a_cyclone_pragma_keylist_reaches_the_same_field_as_rtis_keylist() {
    let ast = model("struct Reading { long id; };\n#pragma keylist Reading id\n");
    let any = ast
        .find_by_scoped_name(&scoped(&["Reading"]))
        .expect("Reading");
    let rti = ast
        .facets(any)
        .expect("facets")
        .vendor
        .rti
        .as_ref()
        .expect("an RTI facet");
    assert_eq!(rti.keylist, vec!["id".to_string()]);
}

#[test]
fn the_opendds_pragmas_reach_the_opendds_facet() {
    let ast = model(
        "struct Message { long id; string body; };\n\
         #pragma DCPS_DATA_TYPE \"Message\"\n\
         #pragma DCPS_DATA_KEY Message id\n",
    );
    let any = ast
        .find_by_scoped_name(&scoped(&["Message"]))
        .expect("Message");
    let open_dds = ast
        .facets(any)
        .expect("facets")
        .vendor
        .open_dds
        .as_ref()
        .expect("an OpenDDS facet");
    assert!(open_dds.dcps_data_type);
    assert_eq!(open_dds.dcps_data_keys, vec!["id".to_string()]);
}

/// `DCPS_DATA_KEY` keys the member it names, as `keylist` does: the facet
/// alone made the file mean nothing, and an OpenDDS `.idl` built a type
/// with no key.
#[test]
fn an_opendds_data_key_makes_the_member_a_key() {
    let ast = model(
        "struct Message { long id; string body; };\n\
         #pragma DCPS_DATA_TYPE \"Message\"\n\
         #pragma DCPS_DATA_KEY Message id\n",
    );
    let any = ast
        .find_by_scoped_name(&scoped(&["Message"]))
        .expect("Message");
    let AnyId::Type(id, _) = any else {
        panic!("expected a type");
    };
    let keyed = |name: &str| {
        let member = ast
            .members_of(id)
            .iter()
            .copied()
            .find(|m| ast.member(*m).expect("member").name == name)
            .expect("the member");
        let node = ast.member(member).expect("member");
        ast.facets(AnyId::Member(member, node.node_id))
            .expect("facets")
            .xtypes
            .as_ref()
            .is_some_and(|x| x.is_key)
    };
    assert!(keyed("id"), "the directive names it");
    assert!(!keyed("body"), "and does not name this one");
}

/// OpenDDS's own spelling puts the type and the field in one string.
#[test]
fn an_opendds_data_key_in_one_quoted_string_keys_the_member() {
    let ast = model(
        "module Robot { struct Pose { string id; double x; }; };\n\
         #pragma DCPS_DATA_TYPE \"Robot::Pose\"\n\
         #pragma DCPS_DATA_KEY \"Robot::Pose id\"\n",
    );
    let any = ast
        .find_by_scoped_name(&scoped(&["Robot", "Pose"]))
        .expect("Pose");
    let AnyId::Type(id, _) = any else {
        panic!("expected a type");
    };
    let keyed = |name: &str| {
        let member = ast
            .members_of(id)
            .iter()
            .copied()
            .find(|m| ast.member(*m).expect("member").name == name)
            .expect("the member");
        let node = ast.member(member).expect("member");
        ast.facets(AnyId::Member(member, node.node_id))
            .expect("facets")
            .xtypes
            .as_ref()
            .is_some_and(|x| x.is_key)
    };
    assert!(keyed("id"));
    assert!(!keyed("x"));
}

/// A name the type does not declare, and an element of an array, are
/// reported rather than keying nothing or keying too much. A path into a
/// nested struct (`p.x`) is a key of its own: `keys_and_inheritance.rs`.
#[test]
fn an_opendds_data_key_the_type_cannot_take_is_reported() {
    for (source, needle) in [
        (
            "struct M { long id; };\n#pragma DCPS_DATA_KEY M missing\n",
            "which the type neither declares nor inherits",
        ),
        (
            "struct M { long a[2]; };\n#pragma DCPS_DATA_KEY M a[0]\n",
            "names an element of a collection",
        ),
        (
            "module N { struct M { long id; }; };\n#pragma DCPS_DATA_KEY N id\n",
            "names something that is not a type",
        ),
        (
            "struct M { long id; };\n#pragma DCPS_DATA_KEY M\n",
            "names no member",
        ),
        (
            "struct M { long id; };\n#pragma DCPS_DATA_KEY\n",
            "names neither a type nor a member",
        ),
        (
            "struct M { long id; };\n#pragma DCPS_DATA_TYPE\n",
            "names no type",
        ),
    ] {
        let found = reports(source);
        assert!(
            found.iter().any(|line| line.contains(needle)),
            "{source:?}: {found:?}"
        );
    }
}

#[test]
fn a_pragma_nobody_recognises_is_kept_rather_than_dropped() {
    let ast = model("struct S { long x; };\n#pragma some_vendor_thing alpha beta\n");
    let root = ast.root_module();
    let node_id = ast.module(root).expect("root").node_id;
    let kept = ast
        .facets(AnyId::Module(root, node_id))
        .expect("facets")
        .vendor
        .rti
        .as_ref()
        .map(|rti| rti.unrecognised.clone())
        .unwrap_or_default();
    assert_eq!(
        kept,
        vec![("some_vendor_thing".to_string(), "alpha beta".to_string())]
    );
}

/// Every diagnostic the lowering reported, as `severity code message`.
fn reports(source: &str) -> Vec<String> {
    let parsed = parse(source);
    assert!(
        !parsed.has_errors(),
        "the fixture does not parse: {:?}",
        parsed.diagnostics()
    );
    lower(&parsed.syntax(), "test.idl")
        .diagnostics
        .iter()
        .map(|d| {
            let severity = if d.is_error() { "error" } else { "warning" };
            format!("{severity} {} {}", d.code.as_str(), d.message)
        })
        .collect()
}

/// The pragmas kept verbatim on the root module, in the order written.
fn kept_pragmas(ast: &Ast) -> Vec<(String, String)> {
    let root = ast.root_module();
    let node_id = ast.module(root).expect("root").node_id;
    ast.facets(AnyId::Module(root, node_id))
        .expect("facets")
        .vendor
        .rti
        .as_ref()
        .map(|rti| rti.unrecognised.clone())
        .unwrap_or_default()
}

// -- a pragma nobody interprets is reported ------------------------------

/// The measurement the fix is for: `#pragma
/// this_pragma_does_not_exist_anywhere` produced nought errors, nought
/// warnings and exit zero, which made it indistinguishable from a
/// directive the front end acts on.
#[test]
fn a_pragma_nobody_interprets_is_reported_rather_than_passed_over() {
    let found = reports("struct S { long x; };\n#pragma this_pragma_does_not_exist_anywhere\n");
    assert_eq!(
        found,
        vec![
            "warning IDL0006 `#pragma this_pragma_does_not_exist_anywhere` is not a directive \
             this front end interprets; it is kept verbatim on the model and changes nothing"
                .to_string()
        ]
    );
}

/// DDS-XTypes 1.3 §7.3.1.1.1: "Compilers that do not support them shall
/// ignore them or issue a warning rather than halting with an error."
/// The level is the clause's, so `--strict` does not raise it: the
/// dialect is a question about vendor extensions, and a pragma nobody
/// claims is not one.
#[test]
fn an_uninterpreted_pragma_is_a_warning_under_every_dialect() {
    for dialect in [
        zerodds_idl_parse::Dialect::STRICT,
        zerodds_idl_parse::Dialect::ALL_VENDORS,
    ] {
        let errors = diagnostics_under(
            "struct S { long x; };\n#pragma this_pragma_does_not_exist_anywhere\n",
            dialect,
        );
        assert!(errors.is_empty(), "{dialect:?} reported {errors:?}");
    }
}

/// Reporting it does not stop it being kept: a round trip back to the
/// dialect that wrote it needs the text that was there.
#[test]
fn the_reported_pragma_is_still_kept_verbatim() {
    let ast = model("struct S { long x; };\n#pragma some_vendor_thing alpha beta\n");
    assert_eq!(
        kept_pragmas(&ast),
        vec![("some_vendor_thing".to_string(), "alpha beta".to_string())]
    );
}

/// The directives the front end does act on say nothing here, or the
/// report would be noise rather than a distinction.
#[test]
fn the_pragmas_the_front_end_interprets_are_not_reported() {
    for source in [
        "struct Reading { long id; };\n#pragma keylist Reading id\n",
        "struct M { long id; };\n#pragma DCPS_DATA_TYPE \"M\"\n",
        "struct M { long id; };\n#pragma DCPS_DATA_KEY M id\n",
        "module M { struct S { long x; }; };\n#pragma prefix \"example.com\"\n",
        "struct S { long x; };\n#pragma ID S \"IDL:S:2.0\"\n",
        "struct S { long x; };\n#pragma version S 1.2\n",
    ] {
        let found: Vec<String> = reports(source)
            .into_iter()
            .filter(|line| line.contains("not a directive this front end interprets"))
            .collect();
        assert!(found.is_empty(), "{source:?} reported {found:?}");
    }
}

// -- the DLRL/OpenDDS pragma-name collision -------------------------------
//
// OMG DDS 1.2 §8.2.2 defines `#pragma DCPS_DATA_TYPE "<name>"` and
// `#pragma DCPS_DATA_KEY "<name> <field>"` as two of DLRL's three
// pragmas; OpenDDS's own devguide reuses the identical two spellings for
// an unrelated purpose, the key-marking dialect `the_opendds_pragmas_
// reach_the_opendds_facet` above tests. This front end resolves the
// collision by owning exactly one reading per pragma name here — the
// OpenDDS one, unchanged from before this section existed — and saying so
// in the diagnostic, so a file meant for DLRL is not silently read as an
// OpenDDS key directive with no word about the other meaning.
// `zerodds-idlc dlrl` is the other reading, over the same three pragma
// names, read independently of this crate's facets — see its own tests.

/// `DCPS_DATA_TYPE`/`DCPS_DATA_KEY` name the DLRL reading, not just the
/// OpenDDS one: a file written for DLRL and pushed through `idlc check`
/// or `idlc generate` gets a diagnostic that says where the other meaning
/// lives, rather than one that reads as though OpenDDS were the only
/// directive that spelling could mean.
#[test]
fn the_opendds_pragma_messages_name_the_dlrl_reading_too() {
    for (source, tag) in [
        (
            "struct Sa { long m; };\n#pragma DCPS_DATA_TYPE \"Sa\"\n",
            "DCPS_DATA_TYPE",
        ),
        (
            "struct Sa { long m; };\n#pragma DCPS_DATA_TYPE \"Sa\"\n\
             #pragma DCPS_DATA_KEY Sa m\n",
            "DCPS_DATA_KEY",
        ),
    ] {
        let found = reports(source);
        assert!(
            found.iter().any(|line| line.contains(tag)
                && line.contains("DLRL")
                && line.contains("zerodds-idlc dlrl")),
            "{tag}: {found:?}"
        );
    }
}

/// `DCPS_DLRL_RELATION` has no OpenDDS meaning at all — the third pragma
/// is DLRL's alone. It is named specifically and not folded into the
/// generic "not a directive this front end interprets" message the
/// measurement in `a_pragma_nobody_interprets_is_reported_rather_than_
/// passed_over` is about: a directive this crate knows the *name* of,
/// even though it does not act on it, is a different state from one it
/// has never heard of.
#[test]
fn dcps_dlrl_relation_is_named_specifically_not_the_generic_message() {
    let found = reports(
        "struct Trade { long id; };\nstruct Order { long id; };\n\
         #pragma DCPS_DLRL_RELATION \"Order trades Trade\"\n",
    );
    assert_eq!(
        found,
        vec![
            "warning IDL0006 `#pragma DCPS_DLRL_RELATION` is OMG DDS 1.2 §8's DLRL relation \
             directive; this front end does not turn it into a type-model facet — \
             `zerodds-idlc dlrl` reads it, together with `DCPS_DATA_TYPE` and \
             `DCPS_DATA_KEY`, into DLRL object and home classes"
                .to_string()
        ]
    );
    assert!(
        !found
            .iter()
            .any(|line| line.contains("not a directive this front end interprets")),
        "{found:?}"
    );
}

/// Not a vendor extension: DDS 1.2 §8 is an OMG specification, not a
/// peer's dialect, so `--strict` — "would every vendor accept this" —
/// has no opinion on it either way. The same standing `dds_xtopics`
/// already has, and the opposite of `keylist`/`DCPS_DATA_TYPE`/
/// `DCPS_DATA_KEY`, which `strict_rejects_every_vendor_extension_the_
/// front_end_acts_on` above holds to an error under `--strict`.
#[test]
fn dcps_dlrl_relation_is_a_warning_under_every_dialect() {
    for dialect in [
        zerodds_idl_parse::Dialect::STRICT,
        zerodds_idl_parse::Dialect::ALL_VENDORS,
    ] {
        let errors = diagnostics_under(
            "struct Trade { long id; };\nstruct Order { long id; };\n\
             #pragma DCPS_DLRL_RELATION \"Order trades Trade\"\n",
            dialect,
        );
        assert!(errors.is_empty(), "{dialect:?} reported {errors:?}");
    }
}

/// Reported and still kept verbatim, the same pairing
/// `the_reported_pragma_is_still_kept_verbatim` holds `#pragma
/// some_vendor_thing` to: a round trip back to a DLRL-aware reader needs
/// the text that was there, and `idlc dlrl`'s own scan reads the *source*
/// rather than this crate's kept-pragma bag — but nothing here should
/// make the pragma vanish from either.
#[test]
fn the_dlrl_relation_pragma_is_still_kept_verbatim() {
    let ast = model(
        "struct Trade { long id; };\nstruct Order { long id; };\n\
         #pragma DCPS_DLRL_RELATION \"Order trades Trade\"\n",
    );
    assert_eq!(
        kept_pragmas(&ast),
        vec![(
            "DCPS_DLRL_RELATION".to_string(),
            "\"Order trades Trade\"".to_string()
        )]
    );
}

// -- §7.3.1.1.2: `#pragma dds_xtopics begin` / `end` ----------------------

/// The clause's own example, character for character apart from the
/// blank lines the specification's page break left in it.
#[test]
fn the_clauses_own_example_is_read_without_a_word() {
    let found = reports(
        "#pragma dds_xtopics begin 1.0_Beta_1\n\
         struct Base { @key long id; };\n\
         #pragma dds_xtopics begin 1.1\n\
         struct Sub { long another_member; };\n\
         #pragma dds_xtopics end 1.1\n\
         #pragma dds_xtopics end 1.0_Beta_1\n",
    );
    assert!(found.is_empty(), "{found:?}");
}

/// "It shall be interpreted without respect to case, and any spaces
/// (for example, in `1.0 Beta 1`) shall be replaced with underscores."
#[test]
fn a_version_is_compared_without_case_and_with_its_spaces_replaced() {
    let found = reports(
        "#pragma dds_xtopics begin 1.0 Beta 1\n\
         struct S { long x; };\n\
         #pragma dds_xtopics end 1.0_BETA_1\n",
    );
    assert!(found.is_empty(), "{found:?}");
}

/// "If version numbers are used with `end` declarations, those version
/// numbers should be the same as those used with the matching `begin`
/// declarations."
#[test]
fn a_version_that_does_not_match_its_begin_is_reported() {
    let found = reports(
        "#pragma dds_xtopics begin 1.1\n\
         struct S { long x; };\n\
         #pragma dds_xtopics end 1.3\n",
    );
    assert_eq!(
        found,
        vec![
            "warning IDL0006 `#pragma dds_xtopics end 1.3` closes a region opened as \
             `begin 1.1`; §7.3.1.1.2 has the two version numbers the same"
                .to_string()
        ]
    );
}

/// The version number is optional in both forms, so a pair that names
/// it on one side only has nothing to compare.
#[test]
fn a_region_that_names_no_version_on_one_side_is_not_a_mismatch() {
    for source in [
        "#pragma dds_xtopics begin\nstruct S { long x; };\n#pragma dds_xtopics end 1.3\n",
        "#pragma dds_xtopics begin 1.3\nstruct S { long x; };\n#pragma dds_xtopics end\n",
        "#pragma dds_xtopics begin\nstruct S { long x; };\n#pragma dds_xtopics end\n",
    ] {
        let found = reports(source);
        assert!(found.is_empty(), "{source:?} reported {found:?}");
    }
}

/// "In the event that such a pragma `begin` declaration is not matched
/// with a subsequent closing `end` declaration, the `begin` declaration
/// shall be considered to continue until the end of the IDL input."
/// A complete region, and not a defect.
#[test]
fn a_begin_with_no_end_runs_to_the_end_of_the_input() {
    let found = reports("#pragma dds_xtopics begin 1.3\nstruct S { long x; };\n");
    assert!(found.is_empty(), "{found:?}");
}

/// The other way round has no reading in the clause at all.
#[test]
fn an_end_with_no_begin_is_reported() {
    let found = reports("struct S { long x; };\n#pragma dds_xtopics end 1.3\n");
    assert_eq!(
        found,
        vec![
            "warning IDL0006 `#pragma dds_xtopics end` closes a region no `begin` opened"
                .to_string()
        ]
    );
}

/// Nesting is read off the order the declarations were written in, so
/// the inner `end` closes the inner `begin` and not the outer one.
#[test]
fn the_inner_end_closes_the_inner_begin() {
    let found = reports(
        "#pragma dds_xtopics begin 1.1\n\
         #pragma dds_xtopics begin 1.3\n\
         struct S { long x; };\n\
         #pragma dds_xtopics end 1.1\n\
         #pragma dds_xtopics end 1.3\n",
    );
    assert_eq!(
        found.len(),
        2,
        "the two ends did not each meet the wrong begin: {found:?}"
    );
    assert!(
        found[0].contains("end 1.1` closes a region opened as `begin 1.3`"),
        "{found:?}"
    );
    assert!(
        found[1].contains("end 1.3` closes a region opened as `begin 1.1`"),
        "{found:?}"
    );
}

/// §7.3.1.1.2 defines the directive in two forms. Anything else written
/// after it — the form `docs/OPEN-ITEMS.md` reported, `#pragma
/// dds_xtopics LegacyTopic`, among them — is neither, and used to be
/// indistinguishable from both.
#[test]
fn a_dds_xtopics_that_is_neither_begin_nor_end_is_reported() {
    let found = reports("struct LegacyTopic { long id; };\n#pragma dds_xtopics LegacyTopic\n");
    assert_eq!(
        found,
        vec![
            "warning IDL0006 `#pragma dds_xtopics LegacyTopic` is neither `begin` nor `end`; \
             §7.3.1.1.2 defines the directive in those two forms only"
                .to_string()
        ]
    );
}

#[test]
fn a_bare_dds_xtopics_is_reported() {
    let found = reports("struct S { long x; };\n#pragma dds_xtopics\n");
    assert_eq!(
        found,
        vec![
            "warning IDL0006 `#pragma dds_xtopics` was written with neither `begin` nor `end`; \
             §7.3.1.1.2 defines the directive in those two forms only"
                .to_string()
        ]
    );
}

/// The declarations are "informative only", so the region reaches no
/// declaration — and the model still has to be able to write it back.
#[test]
fn a_dds_xtopics_region_is_kept_verbatim_and_reaches_no_declaration() {
    let ast = model(
        "#pragma dds_xtopics begin 1.3\n\
         struct S { long x; };\n\
         #pragma dds_xtopics end 1.3\n",
    );
    assert_eq!(
        kept_pragmas(&ast),
        vec![
            ("dds_xtopics".to_string(), "begin 1.3".to_string()),
            ("dds_xtopics".to_string(), "end 1.3".to_string()),
        ]
    );
    let any = ast.find_by_scoped_name(&scoped(&["S"])).expect("S");
    let facets = ast.facets(any).expect("facets");
    assert!(
        facets.vendor.is_empty(),
        "the region reached the declaration: {:?}",
        facets.vendor
    );
}

// -- §7.4.14 rule (114): a directive naming its own scope ----------------

/// The repository id a named declaration ended up with.
fn repository_id_of(ast: &Ast, segments: &[&str]) -> String {
    let any = ast
        .find_by_scoped_name(&scoped(segments))
        .unwrap_or_else(|_| panic!("{segments:?} is not in the model"));
    ast.facets(any)
        .and_then(|facets| facets.corba.as_ref())
        .and_then(|corba| corba.repository_id.clone())
        .unwrap_or_else(|| panic!("{segments:?} has no repository id"))
        .0
}

#[test]
fn a_typeprefix_may_name_the_module_it_is_written_in() {
    // (114). The directive's target is a name being *used*, so §7.5
    // resolves it from the scope that writes it outwards. Qualifying it
    // with the current scope and stopping there made this name `m::m` —
    // a scope that does not exist — and the directive was reported as
    // naming nothing while `::m` sat one step out.
    let parsed = parse("module m { typeprefix m \"example.com\"; struct S { long x; }; };");
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let lowered = lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    assert_eq!(
        repository_id_of(&lowered.ast, &["m", "S"]),
        "IDL:example.com/m/S:1.0"
    );
}

#[test]
fn the_root_anchored_spelling_names_the_same_module() {
    let parsed = parse("module m { typeprefix ::m \"example.com\"; struct S { long x; }; };");
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let lowered = lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    assert_eq!(
        repository_id_of(&lowered.ast, &["m", "S"]),
        "IDL:example.com/m/S:1.0"
    );
}

#[test]
fn a_sibling_declaration_wins_over_one_further_out() {
    // The candidates are innermost first, which is what §7.5 says and
    // what every other name in the language does.
    let parsed = parse(
        "module m { struct S { long x; }; };\n\
         module outer { module m { typeprefix m \"inner.example\"; struct S { long y; }; }; };",
    );
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let lowered = lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    assert_eq!(
        repository_id_of(&lowered.ast, &["outer", "m", "S"]),
        "IDL:inner.example/outer/m/S:1.0"
    );
    // The outer `m` is untouched: the directive never reached it.
    assert_eq!(repository_id_of(&lowered.ast, &["m", "S"]), "IDL:m/S:1.0");
}

#[test]
fn a_directive_naming_nothing_is_still_reported() {
    // The rule the fix must not lose. Looking outwards must not turn a
    // name that denotes nothing into a name that denotes the root.
    let parsed = parse("module m { typeprefix Nowhere \"example.com\"; };");
    let lowered = lower(&parsed.syntax(), "test.idl");
    assert!(
        lowered
            .diagnostics
            .iter()
            .any(|d| d.code == zerodds_idl_parse::DiagnosticCode::UnresolvedName),
        "{:?}",
        lowered.diagnostics
    );
}

// -- §7.4.12 rule (188): a `sequence<>` formal parameter -----------------

#[test]
fn a_sequence_type_formal_parameter_parses_and_carries_its_element() {
    // (188)'s last alternative is a whole `<sequence_type>`, not the
    // bare `sequence` classifier beside it. Only the keyword parsed, so
    // `module M<sequence<long> S> { … };` failed on the `<`.
    let source = "module M<sequence<long> S> { };";
    let parsed = parse(source);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    assert_eq!(parsed.text(), source, "the round trip changed");

    let ast = model(source);
    let module = ast.module_by_path(&scoped(&["M"])).expect("the module");
    let node_id = ast.module(module).expect("module").node_id;
    let params = ast
        .facets(AnyId::Module(module, node_id))
        .expect("facets")
        .corba
        .as_ref()
        .map(|c| c.template_params.clone())
        .unwrap_or_default();
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].name, "S");
    assert_eq!(params[0].kind, TemplateParamKind::Sequence);
    // The element the parameter constrains is kept, which is the whole
    // difference between this alternative and the bare keyword.
    let constraint = params[0]
        .constraint
        .as_ref()
        .and_then(zerodds_ast_core::Ref::resolved)
        .expect("a sequence parameter with no sequence behind it");
    assert!(matches!(
        ast.type_(constraint).expect("the constraint").kind,
        zerodds_ast::TypeKind::Sequence { .. }
    ));
}

#[test]
fn the_bare_sequence_classifier_still_parses_as_itself() {
    // Both alternatives are in the rule and both have to stay.
    let ast = model("module M<sequence S> { };");
    let module = ast.module_by_path(&scoped(&["M"])).expect("the module");
    let node_id = ast.module(module).expect("module").node_id;
    let params = ast
        .facets(AnyId::Module(module, node_id))
        .expect("facets")
        .corba
        .as_ref()
        .map(|c| c.template_params.clone())
        .unwrap_or_default();
    assert_eq!(
        params.first().map(|p| p.kind),
        Some(TemplateParamKind::Sequence)
    );
    assert!(
        params[0].constraint.is_none(),
        "the bare classifier constrains nothing"
    );
}

#[test]
fn a_bounded_sequence_parameter_keeps_its_bound() {
    let source = "module M<sequence<long, 8> S> { };";
    let parsed = parse(source);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    assert_eq!(parsed.text(), source);
}

// -- §7.4.12 rules (190)–(194): instantiation ----------------------------

/// Every named type in a model, by scoped name.
fn type_names(ast: &Ast) -> Vec<String> {
    let mut out: Vec<String> = ast
        .types()
        .filter(|(_, node)| !node.name.is_empty())
        .filter_map(|(id, _)| {
            ast.any_of_type(id)
                .ok()
                .and_then(|any| ast.scoped_name_of(any))
                .map(|name| name.segments().join("::"))
        })
        .collect();
    out.sort();
    out
}

#[test]
fn a_template_module_declares_no_types_of_its_own() {
    // §7.4.12: a template module's contents are definitions *of a
    // template*, and a member whose type is a formal parameter has no
    // type until an actual one is given. Lowering the body in place left
    // `S.a` `Ref::Pending` for ever, with no diagnostic — the block
    // reported success on a model that was not one.
    let ast = model("module M<typename T> { struct S { T a; }; };");
    assert!(
        !type_names(&ast).iter().any(|name| name.ends_with("S")),
        "the template's body reached the model: {:?}",
        type_names(&ast)
    );
    // The declaration itself is still there, with its parameters.
    assert!(ast.module_by_path(&scoped(&["M"])).is_some());
}

#[test]
fn an_instantiation_lowers_the_body_with_the_actual_substituted() {
    // (190) `<template_module_inst> ::= "module" <scoped_name> "<"
    // <actual_parameters> ">" <identifier>` — not parsed at all before.
    let source = "module M<typename T> { struct S { T a; }; };\nmodule M<long> N;";
    let parsed = parse(source);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    assert_eq!(parsed.text(), source, "the round trip changed");

    let mut lowered = lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    lowered.ast.resolve_all_by_name().expect("resolve");

    let s = lowered
        .ast
        .find_by_scoped_name(&scoped(&["N", "S"]))
        .ok()
        .and_then(AnyId::as_type)
        .expect("N::S is not in the model");
    let member = lowered.ast.members_of(s).first().copied().expect("a");
    let target = lowered
        .ast
        .member(member)
        .expect("a")
        .type_
        .resolved()
        .expect("the formal parameter never resolved");
    assert_eq!(
        lowered.ast.type_(target).expect("the actual").kind,
        zerodds_ast::TypeKind::Primitive(zerodds_ast::PrimitiveKind::Int32)
    );
}

#[test]
fn two_instantiations_of_one_template_are_two_independent_modules() {
    let mut lowered = lower(
        &parse(
            "module M<typename T> { struct S { T a; }; };\nmodule M<long> N;\nmodule M<short> P;",
        )
        .syntax(),
        "test.idl",
    );
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    lowered.ast.resolve_all_by_name().expect("resolve");
    let names = type_names(&lowered.ast);
    assert!(names.contains(&"N::S".to_string()), "{names:?}");
    assert!(names.contains(&"P::S".to_string()), "{names:?}");
}

#[test]
fn a_const_actual_parameter_becomes_a_constant_of_the_instance() {
    // (192) `<actual_parameter> ::= <type_spec> | <const_expr>`, and a
    // `const` formal's actual is the second. A bound written as
    // `sequence<T, Capacity>` in the body then resolves the way any
    // other named constant does.
    let source = "module Buffer<typename T, const long Capacity> {\n\
                  struct Bag { sequence<T, Capacity> items; };\n\
                  };\n\
                  module Buffer<short, 8> ShortBuffer;";
    let mut lowered = lower(&parse(source).syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    lowered.ast.resolve_all_by_name().expect("resolve");
    assert!(
        type_names(&lowered.ast).contains(&"ShortBuffer::Bag".to_string()),
        "{:?}",
        type_names(&lowered.ast)
    );
}

#[test]
fn an_alias_hands_the_enclosing_templates_parameter_on() {
    // (193) `<template_module_ref> ::= "alias" <scoped_name> "<"
    // <formal_parameter_names> ">" <identifier>`, and (194) makes those
    // names the enclosing template's own formals rather than types.
    let source = "module Inner<typename T> { struct Held { T v; }; };\n\
                  module Outer<typename U> { alias Inner<U> Nested; struct S { U a; }; };\n\
                  module Outer<long> Concrete;";
    let parsed = parse(source);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let mut lowered = lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    lowered.ast.resolve_all_by_name().expect("resolve");
    let names = type_names(&lowered.ast);
    assert!(names.contains(&"Concrete::S".to_string()), "{names:?}");
    assert!(
        names.contains(&"Concrete::Nested::Held".to_string()),
        "{names:?}"
    );
}

#[test]
fn an_instantiation_of_a_template_nobody_declared_is_reported_by_name() {
    for (source, fragment) in [
        ("module M<long> N;", "not a template module"),
        ("alias M<T> R;", "not a template module"),
    ] {
        let lowered = lower(&parse(source).syntax(), "test.idl");
        let reported = lowered
            .diagnostics
            .iter()
            .find(|d| d.code == zerodds_idl_parse::DiagnosticCode::UnresolvedName)
            .unwrap_or_else(|| panic!("{source:?} said nothing: {:?}", lowered.diagnostics));
        assert!(reported.message.contains(fragment), "{reported}");
    }
}

#[test]
fn an_instantiation_with_the_wrong_number_of_actuals_is_reported() {
    let lowered = lower(
        &parse("module M<typename T, typename U> { struct S { T a; }; };\nmodule M<long> N;")
            .syntax(),
        "test.idl",
    );
    assert!(
        lowered.diagnostics.iter().any(|d| d
            .message
            .contains("takes 2 actual parameter(s) and was given 1")),
        "{:?}",
        lowered.diagnostics
    );
}

#[test]
fn a_plain_module_is_untouched_by_the_instantiation_lookahead() {
    // The lookahead reads past the `<…>` to decide, so every ordinary
    // module has to keep parsing as one.
    for source in [
        "module M { struct S { long a; }; };",
        "module M { module N { struct S { long a; }; }; };",
        "module M { typedef sequence<sequence<long>> Nested; };",
    ] {
        let parsed = parse(source);
        assert!(
            !parsed.has_errors(),
            "{source:?}: {:?}",
            parsed.diagnostics()
        );
        assert_eq!(parsed.text(), source);
    }
}

// -- ROS 2 dialect ---------------------------------------------------------

/// Lower `source` under a dialect and hand back the error diagnostics.
fn errors_under(source: &str, dialect: zerodds_idl_parse::Dialect) -> Vec<String> {
    let parsed = zerodds_idl_parse::parse_with(source, dialect);
    let lowered = zerodds_idl_lower::lower_with(
        &parsed.syntax(),
        "test.idl",
        zerodds_idl_lower::Options {
            dialect,
            ..zerodds_idl_lower::Options::default()
        },
    );
    parsed
        .diagnostics()
        .iter()
        .chain(lowered.diagnostics.iter())
        .filter(|d| d.severity == zerodds_idl_parse::Severity::Error)
        .map(|d| format!("{} {}", d.code.as_str(), d.message))
        .collect()
}

/// Strict rejects the ROS 2 pattern `struct S { Pose pose; };`.
#[test]
fn strict_rejects_ros2_case_collision() {
    let source = "struct Pose { long x; };\nstruct PoseStamped { Pose pose; };\n";
    let errors = errors_under(source, zerodds_idl_parse::Dialect::STRICT);
    assert!(
        errors
            .iter()
            .any(|e| e.contains("IDL0008") && e.contains("Pose") && e.contains("pose")),
        "strict must reject `Pose pose;`, got {errors:?}"
    );
}

/// ROS 2 dialect admits the member/type case collision in the same declaration.
#[test]
fn ros2_dialect_admits_member_type_case_collision() {
    let source = "struct Pose { long x; };\nstruct PoseStamped { Pose pose; };\n";
    let mut dialect = zerodds_idl_parse::Dialect::ALL_VENDORS;
    dialect.ros2 = true;
    let errors = errors_under(source, dialect);
    assert!(
        errors.is_empty(),
        "ROS 2 dialect must admit `Pose pose;`, got {errors:?}"
    );
}

/// ROS 2 dialect admits the parameter/type case collision in the same operation.
#[test]
fn ros2_dialect_admits_parameter_type_case_collision() {
    let source = "interface I { struct Pose { long x; }; void op(in Pose pose); };\n";
    let mut dialect = zerodds_idl_parse::Dialect::ALL_VENDORS;
    dialect.ros2 = true;
    let errors = errors_under(source, dialect);
    assert!(
        errors.is_empty(),
        "ROS 2 dialect must admit `void op(in Pose pose);`, got {errors:?}"
    );
}

/// ROS 2 dialect still rejects two members differing only in case.
#[test]
fn ros2_dialect_rejects_two_members_case_collision() {
    let source = "struct S { long pose; long Pose; };\n";
    let mut dialect = zerodds_idl_parse::Dialect::ALL_VENDORS;
    dialect.ros2 = true;
    let errors = errors_under(source, dialect);
    assert!(
        errors
            .iter()
            .any(|e| e.contains("IDL0008") && e.contains("pose") && e.contains("Pose")),
        "ROS 2 dialect must reject two members differing only in case, got {errors:?}"
    );
}

/// ROS 2 dialect still rejects a member colliding with a type used in a different member.
#[test]
fn ros2_dialect_rejects_cross_member_case_collision() {
    let source = "struct Pose { long x; };\nstruct Other { long y; };\nstruct S { Pose pose; Other Pose; };\n";
    let mut dialect = zerodds_idl_parse::Dialect::ALL_VENDORS;
    dialect.ros2 = true;
    let errors = errors_under(source, dialect);
    assert!(
        errors
            .iter()
            .any(|e| e.contains("IDL0008") && e.contains("Pose") && e.contains("pose")),
        "ROS 2 dialect must reject cross-member collision, got {errors:?}"
    );
}

/// ROS 2 dialect still rejects a union case colliding with its discriminator type.
#[test]
fn ros2_dialect_rejects_union_case_discriminator_collision() {
    let source =
        "enum Kind { A, B };\nunion U switch (Kind) { case A: long kind; case B: short b; };\n";
    let mut dialect = zerodds_idl_parse::Dialect::ALL_VENDORS;
    dialect.ros2 = true;
    let errors = errors_under(source, dialect);
    assert!(
        errors
            .iter()
            .any(|e| e.contains("IDL0008") && e.contains("Kind") && e.contains("kind")),
        "ROS 2 dialect must reject union case/discriminator collision, got {errors:?}"
    );
}
