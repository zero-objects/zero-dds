// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Annex A (normative), "IDL3+ of DDS-DCPS Ports and Connectors": the IDL
//! of the annex (`idl/ccm_dds.idl`) through the IDL frontend, the template
//! module `CCM_DDS::Typed` instantiated for a data type, the port types and
//! connectors through the equivalent-IDL transformation, and the Rust
//! interfaces of this crate against the IDL.
//!
//! Annex B (normative), "IDL for DDS-DLRL Ports and Connectors": the IDL of
//! the annex (`idl/ccm_dds_dlrl.idl`) through the IDL frontend over the
//! generic DLRL IDL of `zerodds-dlrl`, against `DDS::Cache` and against the
//! Rust trait `CacheOperation`.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::path::PathBuf;

use zerodds_ast::{
    AnyId, Ast, ComponentRole, InterfaceKind, InterfaceNode, ParamDirection, TypeKind,
};
use zerodds_ast_core::{Id, Ref, ScopedName};
use zerodds_ccm::dds4ccm::{BasicPortDirection, ConnectorKind, ExtendedPortKind};
use zerodds_idl_compose::{PathLoader, compose_source};

/// The model of `ccm_dds.idl` followed by `more`.
fn model(more: &str) -> Ast {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/idl");
    let source = format!("#include \"ccm_dds.idl\"\n{more}");
    // The unit stands beside the file of the annex, so the quoted includes
    // (`ccm_dds.idl`, and its `dds_rtf2_dcps.idl`) are found there.
    let composed = compose_source(
        &format!("{dir}/annex_a_test.idl"),
        &source,
        &PathLoader::bare(),
    );
    assert!(
        composed.diagnostics.is_empty(),
        "the IDL of Annex A does not compile: {:#?}",
        composed.diagnostics
    );
    composed.ast
}

/// A data type and the instantiation of the template module for it, as a
/// user of the annex writes them.
const FOO: &str = "struct Foo { long id; long value; };
                   typedef sequence<Foo> FooSeq;
                   module CCM_DDS::Typed<Foo, FooSeq> Foo_conn;";

fn path(name: &str) -> ScopedName {
    ScopedName::absolute(name.split("::").map(ToString::to_string).collect())
}

fn interface<'a>(ast: &'a Ast, name: &str) -> (Id<InterfaceNode>, &'a InterfaceNode) {
    match ast.find_by_scoped_name(&path(name)) {
        Ok(AnyId::Interface(id, _)) => (id, ast.interface(id).unwrap()),
        other => panic!("{name} is not an interface: {other:?}"),
    }
}

fn scoped_name(ast: &Ast, any: AnyId) -> String {
    ast.scoped_name_of(any)
        .map(|name| name.segments().join("::"))
        .unwrap_or_default()
}

fn type_name(ast: &Ast, reference: &Ref<zerodds_ast::TypeNode>) -> String {
    let id = reference
        .resolved()
        .unwrap_or_else(|| panic!("an unresolved type: {reference:?}"));
    let node = ast.type_(id).unwrap();
    match &node.kind {
        // An interface used as a type is an object reference to it.
        TypeKind::ObjectRef {
            interface: Some(interface),
        } => {
            let interface = interface.resolved().expect("the interface");
            let node = ast.interface(interface).unwrap();
            scoped_name(ast, AnyId::Interface(interface, node.node_id))
        }
        TypeKind::Primitive(_) => node.name.clone(),
        // The annex has no bounded and no wide string.
        TypeKind::String { .. } => "string".to_string(),
        _ => scoped_name(ast, AnyId::Type(id, node.node_id)),
    }
}

fn exception_names(ast: &Ast, raises: &[Ref<zerodds_ast::ExceptionNode>]) -> Vec<String> {
    raises
        .iter()
        .map(|reference| {
            let id = reference.resolved().expect("a raised exception resolves");
            let node = ast.exception(id).unwrap();
            scoped_name(ast, AnyId::Exception(id, node.node_id))
        })
        .collect()
}

/// An operation as `name(direction type, …) raises (…)`.
fn operations(ast: &Ast, name: &str) -> Vec<String> {
    let (_, node) = interface(ast, name);
    node.operations
        .iter()
        .map(|id| {
            let op = ast.operation(*id).unwrap();
            let params: Vec<String> = op
                .params
                .iter()
                .map(|id| {
                    let param = ast.param(*id).unwrap();
                    let direction = match param.direction {
                        ParamDirection::In => "in",
                        ParamDirection::Out => "out",
                        ParamDirection::InOut => "inout",
                    };
                    format!("{direction} {}", type_name(ast, &param.type_))
                })
                .collect();
            let result = op
                .return_type
                .as_ref()
                .map_or_else(|| "void".to_string(), |r| type_name(ast, r));
            let raises = exception_names(ast, &op.raises);
            let raises = if raises.is_empty() {
                String::new()
            } else {
                format!(" raises ({})", raises.join(", "))
            };
            format!("{result} {}({}){raises}", op.name, params.join(", "))
        })
        .collect()
}

/// The exports of an interface, a port type or a connector that are no
/// operations, in the words of the IDL: `uses I name`, `provides I name`,
/// `mirrorport P name`, `port P name`, `attribute T name`, each with its
/// `setraises`.
fn exports(ast: &Ast, name: &str) -> Vec<String> {
    let (_, node) = interface(ast, name);
    node.attributes
        .iter()
        .map(|id| {
            let attribute = ast.attribute(*id).unwrap();
            let role = attribute
                .facets
                .corba
                .as_ref()
                .and_then(|corba| corba.component_role.clone());
            let of = |interface: &Ref<InterfaceNode>| {
                let id = interface.resolved().expect("the interface of a port");
                let node = ast.interface(id).unwrap();
                scoped_name(ast, AnyId::Interface(id, node.node_id))
            };
            match role {
                Some(ComponentRole::Receptacle {
                    interface,
                    multiple,
                    ..
                }) => {
                    assert!(!multiple, "{name}::{}", attribute.name);
                    format!("uses {} {}", of(&interface), attribute.name)
                }
                Some(ComponentRole::Facet { interface, .. }) => {
                    format!("provides {} {}", of(&interface), attribute.name)
                }
                Some(ComponentRole::ConnectorRole) => {
                    format!(
                        "mirrorport {} {}",
                        type_name(ast, &attribute.type_),
                        attribute.name
                    )
                }
                Some(ComponentRole::PortTypeRole) => {
                    format!(
                        "port {} {}",
                        type_name(ast, &attribute.type_),
                        attribute.name
                    )
                }
                Some(other) => panic!("{name}::{}: {other:?}", attribute.name),
                None => {
                    let readonly = if attribute.readonly { "readonly " } else { "" };
                    let raises = exception_names(ast, &attribute.set_raises);
                    let raises = if raises.is_empty() {
                        String::new()
                    } else {
                        format!(" setraises ({})", raises.join(", "))
                    };
                    format!(
                        "{readonly}attribute {} {}{raises}",
                        type_name(ast, &attribute.type_),
                        attribute.name
                    )
                }
            }
        })
        .collect()
}

fn bases(ast: &Ast, name: &str) -> Vec<String> {
    let (_, node) = interface(ast, name);
    node.bases
        .iter()
        .map(|base| {
            let id = base.resolved().expect("a base resolves");
            let node = ast.interface(id).unwrap();
            scoped_name(ast, AnyId::Interface(id, node.node_id))
        })
        .collect()
}

fn members(ast: &Ast, type_: &str) -> Vec<String> {
    let Ok(AnyId::Type(id, _)) = ast.find_by_scoped_name(&path(type_)) else {
        panic!("{type_} is not a type");
    };
    match &ast.type_(id).unwrap().kind {
        TypeKind::Enumeration { .. } => ast
            .enumerators_of(id)
            .iter()
            .map(|e| ast.enumerator(*e).unwrap().name.clone())
            .collect(),
        _ => ast
            .members_of(id)
            .iter()
            .map(|m| {
                let member = ast.member(*m).unwrap();
                format!("{} {}", type_name(ast, &member.type_), member.name)
            })
            .collect(),
    }
}

fn exception_members(ast: &Ast, name: &str) -> Vec<String> {
    let Ok(AnyId::Exception(id, _)) = ast.find_by_scoped_name(&path(name)) else {
        panic!("{name} is not an exception");
    };
    ast.exception(id)
        .unwrap()
        .members
        .iter()
        .map(|m| {
            let member = ast.member(*m).unwrap();
            format!("{} {}", type_name(ast, &member.type_), member.name)
        })
        .collect()
}

/// Annex A, the non-typed part of module `CCM_DDS`: the file compiles, and
/// the model has every type, constant, exception, interface and connector
/// base of the annex with its members, operations and exceptions.
#[test]
fn dds4ccm_annex_a_the_non_typed_part_of_ccm_dds_compiles_to_the_declarations_of_the_annex() {
    let ast = model("");

    // Enums, structs and typedefs.
    assert_eq!(
        members(&ast, "CCM_DDS::AccessStatus"),
        ["FRESH_INFO", "ALREADY_SEEN"]
    );
    assert_eq!(
        members(&ast, "CCM_DDS::InstanceStatus"),
        [
            "INSTANCE_CREATED",
            "INSTANCE_FILTERED_IN",
            "INSTANCE_UPDATED",
            "INSTANCE_FILTERED_OUT",
            "INSTANCE_DELETED"
        ]
    );
    assert_eq!(
        members(&ast, "CCM_DDS::ListenerMode"),
        ["NOT_ENABLED", "ONE_BY_ONE", "MANY_BY_MANY"]
    );
    assert_eq!(
        members(&ast, "CCM_DDS::ReadInfo"),
        [
            "DDS::InstanceHandle_t instance_handle",
            "DDS::Time_t source_timestamp",
            "CCM_DDS::AccessStatus access_status",
            "CCM_DDS::InstanceStatus instance_status"
        ]
    );
    assert_eq!(
        members(&ast, "CCM_DDS::QueryFilter"),
        ["string expression", "DDS::StringSeq parameters"]
    );
    for alias in ["DataNumber_t", "DataNumberSeq", "ReadInfoSeq"] {
        let found = ast.find_by_scoped_name(&path(&format!("CCM_DDS::{alias}")));
        let Ok(AnyId::Type(id, _)) = found else {
            panic!("CCM_DDS::{alias}: {found:?}");
        };
        assert!(matches!(
            ast.type_(id).unwrap().kind,
            TypeKind::Alias { .. }
        ));
    }
    // `const DataNumber_t UNLIMITED = 0;`
    let (_, unlimited) = ast
        .constants()
        .find(|(_, c)| c.name == "UNLIMITED")
        .expect("CCM_DDS::UNLIMITED");
    assert_eq!(
        ast.eval_const(unlimited.expr),
        Ok(zerodds_ast::ConstValue::Integer(0))
    );
    assert_eq!(type_name(&ast, &unlimited.type_), "CCM_DDS::DataNumber_t");

    // Exceptions.
    assert_eq!(
        exception_members(&ast, "CCM_DDS::AlreadyCreated"),
        ["CCM_DDS::DataNumberSeq indexes"]
    );
    assert_eq!(
        exception_members(&ast, "CCM_DDS::NonExistent"),
        ["CCM_DDS::DataNumberSeq indexes"]
    );
    assert_eq!(
        exception_members(&ast, "CCM_DDS::InternalError"),
        [
            "DDS::ReturnCode_t error_code",
            "CCM_DDS::DataNumber_t index"
        ]
    );
    assert!(exception_members(&ast, "CCM_DDS::NonChangeable").is_empty());

    // Interfaces.
    for name in [
        "DataListenerControl",
        "StateListenerControl",
        "ContentFilterSetting",
        "PortStatusListener",
        "ConnectorStatusListener",
    ] {
        let (_, node) = interface(&ast, &format!("CCM_DDS::{name}"));
        assert_eq!(
            node.kind,
            InterfaceKind::Local,
            "{name} is a local interface"
        );
    }
    assert_eq!(
        exports(&ast, "CCM_DDS::DataListenerControl"),
        [
            "attribute CCM_DDS::ListenerMode mode",
            "attribute CCM_DDS::DataNumber_t max_delivered_data"
        ]
    );
    assert_eq!(
        bases(&ast, "CCM_DDS::StateListenerControl"),
        ["CCM_DDS::DataListenerControl"]
    );
    assert_eq!(
        exports(&ast, "CCM_DDS::StateListenerControl"),
        ["attribute boolean is_filter_interpreted"]
    );
    assert_eq!(
        operations(&ast, "CCM_DDS::ContentFilterSetting"),
        ["void set_filter_parameters(in DDS::StringSeq) raises (CCM_DDS::InternalError)"]
    );
    assert_eq!(
        operations(&ast, "CCM_DDS::PortStatusListener"),
        [
            "void on_requested_deadline_missed(in DDS::DataReader, in DDS::RequestedDeadlineMissedStatus)",
            "void on_sample_lost(in DDS::DataReader, in DDS::SampleLostStatus)"
        ]
    );
    assert_eq!(
        operations(&ast, "CCM_DDS::ConnectorStatusListener"),
        [
            "void on_inconsistent_topic(in DDS::Topic, in DDS::InconsistentTopicStatus)",
            "void on_requested_incompatible_qos(in DDS::DataReader, in DDS::RequestedIncompatibleQosStatus)",
            "void on_sample_rejected(in DDS::DataReader, in DDS::SampleRejectedStatus)",
            "void on_offered_deadline_missed(in DDS::DataWriter, in DDS::OfferedDeadlineMissedStatus)",
            "void on_offered_incompatible_qos(in DDS::DataWriter, in DDS::OfferedIncompatibleQosStatus)",
            "void on_unexpected_status(in DDS::Entity, in DDS::StatusKind)"
        ]
    );

    // Connector bases.
    let (_, base) = interface(&ast, "CCM_DDS::DDS_Base");
    assert_eq!(base.kind, InterfaceKind::Connector);
    assert_eq!(
        exports(&ast, "CCM_DDS::DDS_Base"),
        [
            "uses CCM_DDS::ConnectorStatusListener error_listener",
            "attribute DDS::DomainId_t domain_id setraises (CCM_DDS::NonChangeable)",
            "attribute string qos_profile setraises (CCM_DDS::NonChangeable)"
        ]
    );
    assert_eq!(bases(&ast, "CCM_DDS::DDS_TopicBase"), ["CCM_DDS::DDS_Base"]);
    assert_eq!(
        exports(&ast, "CCM_DDS::DDS_TopicBase"),
        [
            "attribute string topic_name setraises (CCM_DDS::NonChangeable)",
            "attribute DDS::StringSeq key_fields setraises (CCM_DDS::NonChangeable)"
        ]
    );

    // The template module declares its two formal parameters and nothing
    // in the model until it is instantiated.
    let typed = ast
        .module_by_path(&path("CCM_DDS::Typed"))
        .expect("module CCM_DDS::Typed");
    let node_id = ast.module(typed).unwrap().node_id;
    let params = ast
        .facets(AnyId::Module(typed, node_id))
        .and_then(|f| f.corba.as_ref().map(|c| c.template_params.clone()))
        .unwrap_or_default();
    let formals: Vec<(&str, zerodds_ast::TemplateParamKind)> =
        params.iter().map(|p| (p.name.as_str(), p.kind)).collect();
    assert_eq!(
        formals,
        [
            ("T", zerodds_ast::TemplateParamKind::TypeName),
            ("TSeq", zerodds_ast::TemplateParamKind::Sequence)
        ]
    );
    assert!(
        ast.find_by_scoped_name(&path("CCM_DDS::Typed::Writer"))
            .is_err()
    );
}

/// Annex A, the typed sub-part: `module CCM_DDS::Typed<Foo, FooSeq>
/// Foo_conn;` gives the seven interfaces for the data type, with the
/// operations, parameters and exceptions of the annex; `T` is `Foo` and
/// `TSeq` is `FooSeq`.
#[test]
fn dds4ccm_annex_a_instantiating_typed_gives_the_interfaces_for_a_data_type() {
    let ast = model(FOO);
    let internal = "raises (CCM_DDS::InternalError)";
    let existent = "raises (CCM_DDS::NonExistent, CCM_DDS::InternalError)";
    let created = "raises (CCM_DDS::AlreadyCreated, CCM_DDS::InternalError)";
    let handle = "in DDS::InstanceHandle_t";

    assert_eq!(
        operations(&ast, "Foo_conn::InstanceHandleManager"),
        [
            format!("DDS::InstanceHandle_t register_instance(in Foo) {internal}"),
            format!("void unregister_instance(in Foo, {handle}) {internal}")
        ]
    );
    assert_eq!(
        bases(&ast, "Foo_conn::Writer"),
        ["Foo_conn::InstanceHandleManager"]
    );
    assert_eq!(
        operations(&ast, "Foo_conn::Writer"),
        [
            format!("void write_one(in Foo, {handle}) {internal}"),
            format!("void write_many(in FooSeq) {internal}")
        ]
    );
    assert_eq!(
        exports(&ast, "Foo_conn::Writer"),
        ["attribute boolean is_coherent_write"]
    );
    assert_eq!(
        bases(&ast, "Foo_conn::Updater"),
        ["Foo_conn::InstanceHandleManager"]
    );
    assert_eq!(
        operations(&ast, "Foo_conn::Updater"),
        [
            format!("void create_one(in Foo, {handle}) {created}"),
            format!("void update_one(in Foo, {handle}) {existent}"),
            format!("void delete_one(in Foo, {handle}) {existent}"),
            format!("void create_many(in FooSeq) {created}"),
            format!("void update_many(in FooSeq) {existent}"),
            format!("void delete_many(in FooSeq) {existent}")
        ]
    );
    assert_eq!(
        exports(&ast, "Foo_conn::Updater"),
        [
            "readonly attribute boolean is_global_scope",
            "attribute boolean is_coherent_write"
        ]
    );
    assert_eq!(
        operations(&ast, "Foo_conn::Reader"),
        [
            format!("void read_last(inout FooSeq, inout CCM_DDS::ReadInfoSeq) {internal}"),
            format!("void read_all(inout FooSeq, inout CCM_DDS::ReadInfoSeq) {internal}"),
            format!("void read_one_last(inout Foo, out CCM_DDS::ReadInfo, {handle}) {existent}"),
            format!(
                "void read_one_all(in Foo, inout FooSeq, inout CCM_DDS::ReadInfoSeq, {handle}) \
                 {existent}"
            )
        ]
    );
    assert_eq!(
        exports(&ast, "Foo_conn::Reader"),
        ["attribute CCM_DDS::QueryFilter query setraises (CCM_DDS::InternalError)"]
    );
    assert_eq!(
        operations(&ast, "Foo_conn::Getter"),
        [
            format!("boolean get_one(out Foo, out CCM_DDS::ReadInfo) {internal}"),
            format!("boolean get_many(inout FooSeq, inout CCM_DDS::ReadInfoSeq) {internal}")
        ]
    );
    assert_eq!(
        exports(&ast, "Foo_conn::Getter"),
        [
            "attribute DDS::Duration_t time_out",
            "attribute CCM_DDS::DataNumber_t max_delivered_data"
        ]
    );
    assert_eq!(
        operations(&ast, "Foo_conn::Listener"),
        [
            "void on_one_data(in Foo, in CCM_DDS::ReadInfo)",
            "void on_many_data(in FooSeq, in CCM_DDS::ReadInfoSeq)"
        ]
    );
    assert_eq!(
        operations(&ast, "Foo_conn::StateListener"),
        [
            "void on_creation(in Foo, in CCM_DDS::ReadInfo)",
            "void on_one_update(in Foo, in CCM_DDS::ReadInfo)",
            "void on_many_updates(in FooSeq, in CCM_DDS::ReadInfoSeq)",
            "void on_deletion(in Foo, in CCM_DDS::ReadInfo)"
        ]
    );
    for name in [
        "InstanceHandleManager",
        "Writer",
        "Updater",
        "Reader",
        "Getter",
        "Listener",
        "StateListener",
    ] {
        let (_, node) = interface(&ast, &format!("Foo_conn::{name}"));
        assert_eq!(node.kind, InterfaceKind::Local, "{name}");
    }

    // A second data type gets interfaces of its own.
    let ast = model(&format!(
        "{FOO}
         struct Bar {{ string name; }};
         typedef sequence<Bar> BarSeq;
         module CCM_DDS::Typed<Bar, BarSeq> Bar_conn;"
    ));
    assert_eq!(
        operations(&ast, "Bar_conn::Listener"),
        [
            "void on_one_data(in Bar, in CCM_DDS::ReadInfo)",
            "void on_many_data(in BarSeq, in CCM_DDS::ReadInfoSeq)"
        ]
    );
    assert_eq!(
        operations(&ast, "Foo_conn::Listener")[0],
        "void on_one_data(in Foo, in CCM_DDS::ReadInfo)"
    );
}

/// Annex A, "DDS Ports" and "Connectors": the six port types with their
/// basic ports and the attribute `filter`, and the connectors `DDS_State`
/// and `DDS_Event` with their mirror ports, in an instance of the template
/// module. The data model of `zerodds-ccm` names the same.
#[test]
fn dds4ccm_annex_a_the_port_types_and_connectors_of_an_instance_are_those_of_the_annex() {
    let ast = model(FOO);
    let filter = "attribute CCM_DDS::QueryFilter filter setraises (CCM_DDS::NonChangeable)";
    let reading = [
        "uses CCM_DDS::ContentFilterSetting filter_config",
        "uses DDS::DataReader dds_entity",
        "provides CCM_DDS::PortStatusListener status",
    ];
    let with = |first: &[&str]| -> Vec<String> {
        let mut all: Vec<String> = first.iter().map(ToString::to_string).collect();
        all.push(filter.to_string());
        all.extend(reading.iter().map(ToString::to_string));
        all
    };
    let expected: [(&str, Vec<String>); 6] = [
        (
            "DDS_Write",
            vec![
                "uses Foo_conn::Writer data".into(),
                "uses DDS::DataWriter dds_entity".into(),
            ],
        ),
        (
            "DDS_Update",
            vec![
                "uses Foo_conn::Updater data".into(),
                "uses DDS::DataWriter dds_entity".into(),
            ],
        ),
        ("DDS_Read", with(&["uses Foo_conn::Reader data"])),
        (
            "DDS_Get",
            with(&[
                "uses Foo_conn::Reader data",
                "uses Foo_conn::Getter fresh_data",
            ]),
        ),
        (
            "DDS_Listen",
            with(&[
                "uses Foo_conn::Reader data",
                "uses CCM_DDS::DataListenerControl data_control",
                "provides Foo_conn::Listener data_listener",
            ]),
        ),
        (
            "DDS_StateListen",
            with(&[
                "uses Foo_conn::Reader data",
                "uses CCM_DDS::StateListenerControl data_control",
                "provides Foo_conn::StateListener data_listener",
            ]),
        ),
    ];
    for (name, ports) in &expected {
        let qualified = format!("Foo_conn::{name}");
        let (_, node) = interface(&ast, &qualified);
        assert_eq!(node.kind, InterfaceKind::PortType, "{name}");
        assert_eq!(&exports(&ast, &qualified), ports, "porttype {name}");
    }

    // The data model of zerodds-ccm against the IDL: the same six port
    // types, each with the same basic ports in the same order.
    let in_template = |idl_name: &str| -> String {
        // `CCM_DDS::Typed::X` of the template is `Foo_conn::X` of the
        // instance; every other name is the same.
        idl_name.replace("CCM_DDS::Typed::", "Foo_conn::")
    };
    assert_eq!(ExtendedPortKind::ALL.len(), expected.len());
    for (kind, (name, ports)) in ExtendedPortKind::ALL.iter().zip(&expected) {
        assert_eq!(kind.idl_name(), *name);
        let modelled: Vec<String> = kind
            .basic_ports()
            .iter()
            .map(|(port, interface, direction)| {
                let word = match direction {
                    BasicPortDirection::Uses => "uses",
                    BasicPortDirection::Provides => "provides",
                };
                format!("{word} {} {port}", in_template(interface.idl_name()))
            })
            .collect();
        let of_idl: Vec<String> = ports
            .iter()
            .filter(|export| !export.starts_with("attribute"))
            .cloned()
            .collect();
        assert_eq!(modelled, of_idl, "{name}");
        assert_eq!(
            kind.has_filter_attribute(),
            ports.iter().any(|export| export == filter),
            "{name}"
        );
    }

    // Connectors.
    let connectors: [(&str, ConnectorKind, Vec<&str>); 2] = [
        (
            "DDS_State",
            ConnectorKind::State,
            vec![
                "mirrorport Foo_conn::DDS_Update observable",
                "mirrorport Foo_conn::DDS_Read passive_observer",
                "mirrorport Foo_conn::DDS_Get pull_observer",
                "mirrorport Foo_conn::DDS_Listen push_observer",
                "mirrorport Foo_conn::DDS_StateListen push_state_observer",
            ],
        ),
        (
            "DDS_Event",
            ConnectorKind::Event,
            vec![
                "mirrorport Foo_conn::DDS_Write supplier",
                "mirrorport Foo_conn::DDS_Get pull_consumer",
                "mirrorport Foo_conn::DDS_Listen push_consumer",
            ],
        ),
    ];
    for (name, kind, mirror_ports) in connectors {
        let qualified = format!("Foo_conn::{name}");
        let (_, node) = interface(&ast, &qualified);
        assert_eq!(node.kind, InterfaceKind::Connector, "{name}");
        assert_eq!(bases(&ast, &qualified), ["CCM_DDS::DDS_TopicBase"]);
        assert_eq!(exports(&ast, &qualified), mirror_ports, "connector {name}");
        // The data model.
        assert_eq!(in_template(kind.idl_name()), qualified);
        assert_eq!(kind.base(), Some(ConnectorKind::TopicBase));
        let modelled: Vec<String> = kind
            .mirror_ports()
            .iter()
            .map(|(port, port_type)| {
                format!("mirrorport Foo_conn::{} {port}", port_type.idl_name())
            })
            .collect();
        assert_eq!(modelled, mirror_ports, "{name}");
    }
    // The connector bases of the data model against the IDL.
    for (kind, name) in [
        (ConnectorKind::Base, "CCM_DDS::DDS_Base"),
        (ConnectorKind::TopicBase, "CCM_DDS::DDS_TopicBase"),
    ] {
        assert_eq!(kind.idl_name(), name);
        let modelled: Vec<String> = kind
            .own_attributes()
            .iter()
            .map(|a| {
                format!(
                    "attribute {} {} setraises (CCM_DDS::NonChangeable)",
                    a.idl_type, a.name
                )
            })
            .collect();
        let of_idl: Vec<String> = exports(&ast, name)
            .into_iter()
            .filter(|export| export.starts_with("attribute"))
            .collect();
        assert_eq!(modelled, of_idl, "{name}");
    }
    let (port, interface_of_port, direction) = ConnectorKind::ERROR_LISTENER;
    assert_eq!(
        format!(
            "{} {} {port}",
            match direction {
                BasicPortDirection::Uses => "uses",
                BasicPortDirection::Provides => "provides",
            },
            interface_of_port.idl_name()
        ),
        exports(&ast, "CCM_DDS::DDS_Base")[0]
    );
}

fn operation_names(ast: &Ast, id: Id<InterfaceNode>) -> Vec<String> {
    ast.interface(id)
        .unwrap()
        .operations
        .iter()
        .map(|op| ast.operation(*op).unwrap().name.clone())
        .collect()
}

fn attribute_names(ast: &Ast, id: Id<InterfaceNode>) -> Vec<String> {
    ast.attributes_of(id)
        .iter()
        .map(|a| ast.attribute(*a).unwrap().name.clone())
        .collect()
}

/// Annex A with the extended ports of IDL 4.2 §7.4.11.4: a component that
/// declares a port of an instantiated port type has the basic ports of
/// that port type (a facet for `provides`, a receptacle for `uses`), and a
/// connector has them mirrored (§7.4.11.4.2).
#[test]
fn dds4ccm_annex_a_a_port_gives_a_component_its_basic_ports_and_a_connector_their_mirror() {
    let mut ast = model(&format!(
        "{FOO}
         component Display {{ port Foo_conn::DDS_Listen track; }};"
    ));

    // The component: `uses` is a receptacle (connect, disconnect,
    // get_connection), `provides` a facet (provide).
    let (display, _) = interface(&ast, "Display");
    zerodds_ccm::transform_component(&mut ast, display).expect("the component maps");
    let ops = operation_names(&ast, display);
    for receptacle in ["data", "data_control", "filter_config", "dds_entity"] {
        for verb in ["connect", "disconnect", "get_connection"] {
            let op = format!("{verb}_track_{receptacle}");
            assert!(ops.contains(&op), "{op} in {ops:?}");
        }
        assert!(!ops.contains(&format!("provide_track_{receptacle}")));
    }
    for facet in ["data_listener", "status"] {
        let op = format!("provide_track_{facet}");
        assert!(ops.contains(&op), "{op} in {ops:?}");
        assert!(!ops.contains(&format!("connect_track_{facet}")));
    }
    assert_eq!(ops.len(), 4 * 3 + 2, "{ops:?}");
    assert_eq!(attribute_names(&ast, display), ["track_filter"]);

    // The connector DDS_State: its five mirror ports turn every receptacle
    // of the port types into a facet and every facet into a receptacle.
    let (state, _) = interface(&ast, "Foo_conn::DDS_State");
    zerodds_ccm::transform_connector(&mut ast, state).expect("the connector maps");
    let ops = operation_names(&ast, state);
    let mut expected: Vec<String> = Vec::new();
    for (port, kind) in ConnectorKind::State.mirror_ports() {
        for (basic, _, direction) in kind.basic_ports() {
            match direction {
                // `uses` of the port type: the connector provides it.
                BasicPortDirection::Uses => expected.push(format!("provide_{port}_{basic}")),
                // `provides` of the port type: the connector uses it.
                BasicPortDirection::Provides => {
                    for verb in ["connect", "disconnect", "get_connection"] {
                        expected.push(format!("{verb}_{port}_{basic}"));
                    }
                }
            }
        }
    }
    let (mut got, mut want) = (ops.clone(), expected);
    got.sort();
    want.sort();
    assert_eq!(got, want);
    // The fragment of the observable provides what the component uses.
    assert!(ops.contains(&"provide_observable_data".to_string()));
    assert!(ops.contains(&"connect_push_observer_data_listener".to_string()));
    assert!(ops.contains(&"connect_passive_observer_status".to_string()));
    // The attribute `filter` of the four reading port types.
    assert_eq!(
        attribute_names(&ast, state),
        [
            "passive_observer_filter",
            "pull_observer_filter",
            "push_observer_filter",
            "push_state_observer_filter"
        ]
    );
}

/// A method of a Rust trait, or the method an IDL operation or attribute
/// maps to: the name, the parameters after `&self` as (name, type) and the
/// result type (`()` for none). The setter of an attribute has the
/// parameter name `_`: the IDL gives it none.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Signature {
    name: String,
    params: Vec<(String, String)>,
    result: String,
}

/// A type as rustfmt writes it on one line: no space after `<`, `(`, `[`
/// and `&`, none before `>`, `)`, `]` and `,`, one after `,`.
fn one_line(text: &str) -> String {
    let words = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let chars: Vec<char> = words.chars().collect();
    let mut out = String::new();
    for (i, c) in chars.iter().enumerate() {
        if *c == ' ' {
            let before = chars.get(i.wrapping_sub(1)).copied().unwrap_or(' ');
            let after = chars.get(i + 1).copied().unwrap_or(' ');
            if "<([&,".contains(before) || ">)],".contains(after) {
                continue;
            }
        }
        out.push(*c);
    }
    out.replace(',', ", ")
}

/// Splits `text` at the commas outside `<>`, `()` and `[]`.
fn top_level_commas(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let (mut depth, mut part) = (0i32, String::new());
    for c in text.chars() {
        match c {
            '<' | '(' | '[' => depth += 1,
            '>' | ')' | ']' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(core::mem::take(&mut part));
                continue;
            }
            _ => {}
        }
        part.push(c);
    }
    parts.push(part);
    parts
        .into_iter()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

/// The methods of `pub trait {name}` in a Rust source text, with their
/// parameters and results. Every method takes `&self`; a trait with a
/// provided method (a body) is refused.
fn trait_signatures(source: &str, name: &str) -> Vec<Signature> {
    let head = format!("pub trait {name}");
    let start = source
        .find(&head)
        .unwrap_or_else(|| panic!("no trait {name}"));
    let body = &source[start..];
    let end = body.find("\n}\n").expect("the end of the trait");
    let code: Vec<&str> = body[..end]
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect();
    let code = code.join(" ");
    let code = &code[code.find('{').expect("the body of the trait") + 1..];
    assert!(!code.contains('{'), "trait {name} has a provided method");
    code.split(';')
        .filter_map(|declaration| declaration.trim().strip_prefix("fn "))
        .map(|declaration| signature_of(declaration, name))
        .collect()
}

/// The signature of a method declared as `declaration`, the text after
/// `fn` up to the `;` of a trait method or the `{` of a body; `owner` names
/// the trait or type in the messages.
fn signature_of(declaration: &str, owner: &str) -> Signature {
    let open = declaration.find('(').expect("the parameter list");
    let method = declaration[..open].trim().to_string();
    assert!(!method.contains('<'), "{owner}::{method} is generic");
    let (mut depth, mut close) = (0i32, open);
    for (i, c) in declaration[open..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            close = open + i;
            break;
        }
    }
    let mut params = top_level_commas(&declaration[open + 1..close]);
    assert_eq!(
        params.first().map(String::as_str),
        Some("&self"),
        "{owner}::{method} takes &self"
    );
    params.remove(0);
    let params = params
        .iter()
        .map(|param| {
            let (param_name, type_) = param.split_once(':').expect("name: type");
            (param_name.trim().to_string(), one_line(type_))
        })
        .collect();
    let rest = declaration[close + 1..].trim();
    let result = rest
        .strip_prefix("->")
        .map_or_else(|| "()".to_string(), one_line);
    Signature {
        name: method,
        params,
        result,
    }
}

/// The Rust spelling of an `in` parameter of the IDL type `idl`. `T` and
/// `TSeq` of the instance `Foo_conn` are `Foo` and `FooSeq`; a sequence is a
/// slice, a structure and an object reference a reference, a handle, a
/// status, a status kind and a string sequence a value.
fn rust_in(idl: &str) -> String {
    match idl {
        "Foo" => "&T".into(),
        "FooSeq" => "&[T]".into(),
        "CCM_DDS::ReadInfo" => "&ReadInfo".into(),
        "CCM_DDS::ReadInfoSeq" => "&[ReadInfo]".into(),
        "DDS::InstanceHandle_t" => "InstanceHandle".into(),
        "DDS::StringSeq" => "Vec<String>".into(),
        "DDS::DataReader" => "&AnyDataReader".into(),
        "DDS::DataWriter" => "&zerodds_dcps::publisher::AnyDataWriter".into(),
        "DDS::Topic" => "&TopicDescriptionHandle".into(),
        "DDS::Entity" => "&DdsEntity".into(),
        "DDS::StatusKind" => "StatusKind".into(),
        status if status.starts_with("DDS::") && status.ends_with("Status") => {
            status["DDS::".len()..].to_string()
        }
        other => panic!("no Rust spelling of an `in` {other}"),
    }
}

/// The Rust spelling of an `inout` or `out` parameter of the IDL type `idl`:
/// `&mut` of the type the operation fills (§7.2.1.3).
fn rust_out(idl: &str) -> String {
    match idl {
        "Foo" => "&mut T".into(),
        "FooSeq" => "&mut Vec<T>".into(),
        "CCM_DDS::ReadInfo" => "&mut ReadInfo".into(),
        "CCM_DDS::ReadInfoSeq" => "&mut ReadInfoSeq".into(),
        other => panic!("no Rust spelling of an `out` {other}"),
    }
}

/// The Rust spelling of a result or an attribute value of the IDL type
/// `idl`.
fn rust_value(idl: &str) -> String {
    match idl {
        "void" => "()".into(),
        "boolean" => "bool".into(),
        "DDS::InstanceHandle_t" => "InstanceHandle".into(),
        "DDS::Duration_t" => "Duration".into(),
        "CCM_DDS::DataNumber_t" => "DataNumber".into(),
        "CCM_DDS::ListenerMode" => "ListenerMode".into(),
        "CCM_DDS::QueryFilter" => "QueryFilter".into(),
        other => panic!("no Rust spelling of a value {other}"),
    }
}

/// The Rust result of a method with the value `value` and the IDL `raises`
/// list `raises`: the value without exceptions, `Result` with the one
/// exception, `Result` with the error enumeration of the trait for more.
fn rust_result(value: String, raises: &[String], rust_trait: &str) -> String {
    match raises {
        [] => value,
        [one] => format!(
            "Result<{value}, {}>",
            one.strip_prefix("CCM_DDS::")
                .expect("an exception of CCM_DDS")
        ),
        _ => {
            let error = match rust_trait {
                "Updater" => "UpdaterError",
                "Reader" => "ReaderError",
                other => panic!("trait {other} has no error enumeration"),
            };
            format!("Result<{value}, {error}>")
        }
    }
}

/// The methods an IDL interface maps to in Rust, by the mapping of this
/// crate (module documentation of `ccm_dds` and `ccm_dds_typed`): an
/// operation is a method of the same name with the parameters of the IDL
/// in their order and of their direction; an attribute `a` is a getter `a`
/// and, unless it is read-only, a setter `set_a` with its `setraises`.
fn idl_signatures(ast: &Ast, name: &str, rust_trait: &str) -> Vec<Signature> {
    let (_, node) = interface(ast, name);
    let mut methods = Vec::new();
    for id in &node.operations {
        let op = ast.operation(*id).unwrap();
        let params = op
            .params
            .iter()
            .map(|id| {
                let param = ast.param(*id).unwrap();
                let type_ = type_name(ast, &param.type_);
                let spelled = match param.direction {
                    ParamDirection::In => rust_in(&type_),
                    ParamDirection::Out | ParamDirection::InOut => rust_out(&type_),
                };
                // IDL identifiers differ by more than case (IDL 4.2 §7.2.3);
                // `The_writer` of the annex is `the_writer`.
                (param.name.to_lowercase(), spelled)
            })
            .collect();
        let value = op
            .return_type
            .as_ref()
            .map_or_else(|| "()".to_string(), |r| rust_value(&type_name(ast, r)));
        methods.push(Signature {
            name: op.name.clone(),
            params,
            result: rust_result(value, &exception_names(ast, &op.raises), rust_trait),
        });
    }
    for id in &node.attributes {
        let attribute = ast.attribute(*id).unwrap();
        let value = rust_value(&type_name(ast, &attribute.type_));
        methods.push(Signature {
            name: attribute.name.clone(),
            params: Vec::new(),
            result: rust_result(
                value.clone(),
                &exception_names(ast, &attribute.get_raises),
                rust_trait,
            ),
        });
        if !attribute.readonly {
            methods.push(Signature {
                name: format!("set_{}", attribute.name),
                params: vec![("_".to_string(), value)],
                result: rust_result(
                    "()".to_string(),
                    &exception_names(ast, &attribute.set_raises),
                    rust_trait,
                ),
            });
        }
    }
    methods.sort();
    methods
}

/// The exceptions the operations of an IDL interface raise, each once.
fn raised_by(ast: &Ast, name: &str) -> Vec<String> {
    let (_, node) = interface(ast, name);
    let mut raised: Vec<String> = node
        .operations
        .iter()
        .flat_map(|id| exception_names(ast, &ast.operation(*id).unwrap().raises))
        .collect();
    raised.sort();
    raised.dedup();
    raised
}

/// Annex A against the Rust interfaces of this crate: every interface of
/// the IDL is a trait of `ccm_dds` or `ccm_dds_typed` with the operations
/// and attributes of the IDL and no other, each with the parameters of the
/// IDL (name, order, direction, type), its result and its exceptions; the
/// error enumerations of the traits have the exceptions of the IDL; the
/// enumerators, structure members and exceptions have the names of the IDL.
#[test]
fn dds4ccm_annex_a_the_rust_interfaces_have_the_operations_of_the_idl() {
    let ast = model(FOO);
    let non_typed = include_str!("../src/ccm_dds.rs");
    let typed = include_str!("../src/ccm_dds_typed.rs");
    let compared: [(&str, &str, &str); 12] = [
        (
            "CCM_DDS::DataListenerControl",
            non_typed,
            "DataListenerControl",
        ),
        (
            "CCM_DDS::StateListenerControl",
            non_typed,
            "StateListenerControl",
        ),
        (
            "CCM_DDS::ContentFilterSetting",
            non_typed,
            "ContentFilterSetting",
        ),
        (
            "CCM_DDS::PortStatusListener",
            non_typed,
            "PortStatusListener",
        ),
        (
            "CCM_DDS::ConnectorStatusListener",
            non_typed,
            "ConnectorStatusListener",
        ),
        (
            "Foo_conn::InstanceHandleManager",
            typed,
            "InstanceHandleManager",
        ),
        ("Foo_conn::Writer", typed, "Writer"),
        ("Foo_conn::Updater", typed, "Updater"),
        ("Foo_conn::Reader", typed, "Reader"),
        ("Foo_conn::Getter", typed, "Getter"),
        ("Foo_conn::Listener", typed, "Listener"),
        ("Foo_conn::StateListener", typed, "StateListener"),
    ];
    for (idl, source, rust) in compared {
        let expected = idl_signatures(&ast, idl, rust);
        let mut of_rust = trait_signatures(source, rust);
        // A setter's parameter has no name in the IDL.
        for method in &mut of_rust {
            let is_setter = expected
                .iter()
                .any(|e| e.name == method.name && e.params.first().is_some_and(|p| p.0 == "_"));
            if is_setter {
                for param in &mut method.params {
                    param.0 = "_".to_string();
                }
            }
        }
        of_rust.sort();
        assert_eq!(of_rust, expected, "trait {rust} against {idl}");
    }
    // The error enumerations of `Updater` and `Reader` have one variant per
    // exception the operations of the interface raise, and no other.
    use zerodds_dds4ccm::ccm_dds::{ReaderError, UpdaterError};
    let updater_variant = |error: &UpdaterError| match error {
        UpdaterError::AlreadyCreated(_) => "CCM_DDS::AlreadyCreated",
        UpdaterError::NonExistent(_) => "CCM_DDS::NonExistent",
        UpdaterError::InternalError(_) => "CCM_DDS::InternalError",
    };
    let reader_variant = |error: &ReaderError| match error {
        ReaderError::NonExistent(_) => "CCM_DDS::NonExistent",
        ReaderError::InternalError(_) => "CCM_DDS::InternalError",
    };
    let internal = zerodds_dds4ccm::InternalError::code(1);
    let none = zerodds_dds4ccm::NonExistent {
        indexes: Vec::new(),
    };
    let mut of_updater = [
        UpdaterError::AlreadyCreated(zerodds_dds4ccm::AlreadyCreated {
            indexes: Vec::new(),
        }),
        UpdaterError::NonExistent(none.clone()),
        UpdaterError::InternalError(internal),
    ]
    .iter()
    .map(updater_variant)
    .collect::<Vec<_>>();
    of_updater.sort_unstable();
    assert_eq!(of_updater, raised_by(&ast, "Foo_conn::Updater"));
    let mut of_reader = [
        ReaderError::NonExistent(none),
        ReaderError::InternalError(internal),
    ]
    .iter()
    .map(reader_variant)
    .collect::<Vec<_>>();
    of_reader.sort_unstable();
    assert_eq!(of_reader, raised_by(&ast, "Foo_conn::Reader"));
    // Every interface of the IDL outside module DDS is in the list above.
    let listed: Vec<&str> = compared.iter().map(|(idl, _, _)| *idl).collect();
    for (id, node) in ast.interfaces() {
        if !matches!(node.kind, InterfaceKind::Local | InterfaceKind::Interface) {
            continue;
        }
        let name = scoped_name(&ast, AnyId::Interface(id, node.node_id));
        assert!(
            name.starts_with("DDS::") || listed.contains(&name.as_str()),
            "{name} of the IDL has no Rust interface in the comparison"
        );
    }

    // The traits are the ones the comparison read: a use of each method
    // that does not compile if the trait loses it.
    use zerodds_dds4ccm::{
        ConnectorStatusListener, ContentFilterSetting, DataListenerControl, Getter,
        InstanceHandleManager, Listener, PortStatusListener, Reader, StateListener,
        StateListenerControl, Updater, Writer,
    };
    type Datum = u32;
    let _ = <dyn DataListenerControl>::set_mode;
    let _ = <dyn StateListenerControl>::set_is_filter_interpreted;
    let _ = <dyn ContentFilterSetting>::set_filter_parameters;
    let _ = <dyn PortStatusListener>::on_sample_lost;
    let _ = <dyn ConnectorStatusListener>::on_unexpected_status;
    let _ = <dyn InstanceHandleManager<Datum>>::register_instance;
    let _ = <dyn Writer<Datum>>::write_many;
    let _ = <dyn Updater<Datum>>::delete_many;
    let _ = <dyn Reader<Datum>>::read_one_all;
    let _ = <dyn Getter<Datum>>::get_many;
    let _ = <dyn Listener<Datum>>::on_many_data;
    let _ = <dyn StateListener<Datum>>::on_many_updates;

    // Enumerators, structure members and exceptions.
    use zerodds_dds4ccm::{
        AccessStatus, AlreadyCreated, InstanceStatus, InternalError, ListenerMode, NonChangeable,
        NonExistent, QueryFilter, ReadInfo, UNLIMITED,
    };
    let shouting = |debug: String| -> String {
        // `InstanceFilteredIn` -> `INSTANCE_FILTERED_IN`
        let mut out = String::new();
        for (i, c) in debug.chars().enumerate() {
            if c.is_uppercase() && i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_uppercase());
        }
        out
    };
    assert_eq!(
        [AccessStatus::FreshInfo, AccessStatus::AlreadySeen].map(|v| shouting(format!("{v:?}"))),
        members(&ast, "CCM_DDS::AccessStatus").as_slice()
    );
    assert_eq!(
        [
            InstanceStatus::InstanceCreated,
            InstanceStatus::InstanceFilteredIn,
            InstanceStatus::InstanceUpdated,
            InstanceStatus::InstanceFilteredOut,
            InstanceStatus::InstanceDeleted
        ]
        .map(|v| shouting(format!("{v:?}"))),
        members(&ast, "CCM_DDS::InstanceStatus").as_slice()
    );
    assert_eq!(
        [
            ListenerMode::NotEnabled,
            ListenerMode::OneByOne,
            ListenerMode::ManyByMany
        ]
        .map(|v| shouting(format!("{v:?}"))),
        members(&ast, "CCM_DDS::ListenerMode").as_slice()
    );
    let field_names = |idl: Vec<String>| -> Vec<String> {
        idl.iter()
            .map(|member| member.rsplit(' ').next().unwrap_or_default().to_string())
            .collect()
    };
    // The structures and exceptions are built by their IDL member names.
    let info = ReadInfo::default();
    let ReadInfo {
        instance_handle: _,
        source_timestamp: _,
        access_status: _,
        instance_status: _,
    } = info;
    assert_eq!(
        field_names(members(&ast, "CCM_DDS::ReadInfo")),
        [
            "instance_handle",
            "source_timestamp",
            "access_status",
            "instance_status"
        ]
    );
    let QueryFilter {
        expression: _,
        parameters: _,
    } = QueryFilter::default();
    assert_eq!(
        field_names(members(&ast, "CCM_DDS::QueryFilter")),
        ["expression", "parameters"]
    );
    let AlreadyCreated { indexes: _ } = AlreadyCreated { indexes: vec![1] };
    let NonExistent { indexes: _ } = NonExistent { indexes: vec![1] };
    assert_eq!(
        field_names(exception_members(&ast, "CCM_DDS::AlreadyCreated")),
        ["indexes"]
    );
    assert_eq!(
        field_names(exception_members(&ast, "CCM_DDS::NonExistent")),
        ["indexes"]
    );
    let InternalError {
        error_code: _,
        index: _,
    } = InternalError {
        error_code: 1,
        index: 0,
    };
    assert_eq!(
        field_names(exception_members(&ast, "CCM_DDS::InternalError")),
        ["error_code", "index"]
    );
    let NonChangeable = NonChangeable;
    assert!(exception_members(&ast, "CCM_DDS::NonChangeable").is_empty());
    assert_eq!(UNLIMITED, 0);
}

// -------------------------------------------------------------------------
// Annex B
// -------------------------------------------------------------------------

/// The directory of the generic DLRL IDL of `zerodds-dlrl`, which
/// `dds_rtf2_dlrl.idl` includes from the include path.
const DLRL_IDL: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../dlrl/idl");

/// `source` composed beside the files of the annexes, with `include` as
/// the include path: the model and the diagnostics.
fn compose_beside_the_annexes(source: &str, include: &[&str]) -> (Ast, Vec<String>) {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/idl");
    let composed = compose_source(
        &format!("{dir}/annex_b_test.idl"),
        source,
        &PathLoader::new(include.iter().map(PathBuf::from).collect()),
    );
    let diagnostics = composed
        .diagnostics
        .iter()
        .map(|d| format!("{d:?}"))
        .collect();
    (composed.ast, diagnostics)
}

/// The model of `source` with the generic DLRL IDL on the include path; it
/// compiles without a diagnostic.
fn model_with_dlrl(source: &str) -> Ast {
    let (ast, diagnostics) = compose_beside_the_annexes(source, &[DLRL_IDL]);
    assert!(
        diagnostics.is_empty(),
        "the IDL of Annex B does not compile: {diagnostics:#?}"
    );
    ast
}

/// The element type of the sequence a typedef names.
fn sequence_element(ast: &Ast, typedef: &str) -> String {
    let Ok(AnyId::Type(id, _)) = ast.find_by_scoped_name(&path(typedef)) else {
        panic!("{typedef} is not a type");
    };
    let TypeKind::Alias { aliased } = &ast.type_(id).unwrap().kind else {
        panic!("{typedef} is not a typedef");
    };
    let aliased = aliased.resolved().expect("the aliased type resolves");
    let TypeKind::Sequence { element, .. } = &ast.type_(aliased).unwrap().kind else {
        panic!("{typedef} is not a sequence");
    };
    type_name(ast, element)
}

/// The operations of an interface with the names of their parameters, as
/// `name(a, b)`.
fn parameter_names(ast: &Ast, name: &str) -> Vec<String> {
    let (_, node) = interface(ast, name);
    node.operations
        .iter()
        .map(|id| {
            let op = ast.operation(*id).unwrap();
            let names: Vec<String> = op
                .params
                .iter()
                .map(|id| ast.param(*id).unwrap().name.clone())
                .collect();
            format!("{}({})", op.name, names.join(", "))
        })
        .collect()
}

/// The attributes and operations an interface declares, by name: an
/// attribute as `exports` writes it, an operation as `operations` writes
/// it and with the names of its parameters.
fn declarations(ast: &Ast, name: &str) -> Vec<(String, String)> {
    let (_, node) = interface(ast, name);
    let attributes = node
        .attributes
        .iter()
        .map(|id| ast.attribute(*id).unwrap().name.clone())
        .zip(exports(ast, name));
    let operations = node
        .operations
        .iter()
        .map(|id| ast.operation(*id).unwrap().name.clone())
        .zip(
            operations(ast, name)
                .into_iter()
                .zip(parameter_names(ast, name))
                .map(|(operation, names)| format!("{operation} / {names}")),
        );
    attributes.chain(operations).collect()
}

/// The declarations of an interface and of its bases.
fn declarations_with_bases(ast: &Ast, name: &str) -> Vec<(String, String)> {
    let mut all = declarations(ast, name);
    for base in bases(ast, name) {
        all.extend(declarations_with_bases(ast, &base));
    }
    all
}

/// Annex B: `ccm_dds_dlrl.idl` compiles over the generic DLRL IDL, and
/// `CCM_DDS::CacheOperation` is the local interface of the annex, with its
/// six read-only attributes and seven operations, their parameters and
/// exceptions, over the types of module `DDS` of the DLRL IDL.
#[test]
fn dds4ccm_annex_b_cache_operation_compiles_to_the_local_interface_of_the_annex() {
    let ast = model_with_dlrl("#include \"ccm_dds_dlrl.idl\"\n");
    let (_, node) = interface(&ast, "CCM_DDS::CacheOperation");
    assert_eq!(node.kind, InterfaceKind::Local);
    assert!(node.bases.is_empty());
    assert_eq!(
        exports(&ast, "CCM_DDS::CacheOperation"),
        [
            "readonly attribute DDS::CacheUsage cache_usage",
            "readonly attribute DDS::ObjectRootSeq objects",
            "readonly attribute boolean updates_enabled",
            "readonly attribute DDS::ObjectHomeSeq homes",
            "readonly attribute DDS::CacheAccessSeq sub_accesses",
            "readonly attribute DDS::CacheListenerSeq listeners",
        ]
    );
    assert_eq!(
        operations(&ast, "CCM_DDS::CacheOperation"),
        [
            "void refresh() raises (DDS::DCPSError)",
            "void attach_listener(in DDS::CacheListener)",
            "void detach_listener(in DDS::CacheListener)",
            "void enable_updates()",
            "void disable_updates()",
            "DDS::CacheAccess create_access(in DDS::CacheUsage) raises (DDS::PreconditionNotMet)",
            "void delete_access(in DDS::CacheAccess) raises (DDS::PreconditionNotMet)",
        ]
    );
    assert_eq!(
        parameter_names(&ast, "CCM_DDS::CacheOperation"),
        [
            "refresh()",
            "attach_listener(listener)",
            "detach_listener(listener)",
            "enable_updates()",
            "disable_updates()",
            "create_access(purpose)",
            "delete_access(access)",
        ]
    );
    // The types of module DDS are those of the generic DLRL IDL.
    assert_eq!(
        members(&ast, "DDS::CacheUsage"),
        ["READ_ONLY", "WRITE_ONLY", "READ_WRITE"]
    );
    for (sequence, element) in [
        ("DDS::ObjectRootSeq", "DDS::ObjectRoot"),
        ("DDS::ObjectHomeSeq", "DDS::ObjectHome"),
        ("DDS::CacheAccessSeq", "DDS::CacheAccess"),
        ("DDS::CacheListenerSeq", "DDS::CacheListener"),
    ] {
        assert_eq!(sequence_element(&ast, sequence), element, "{sequence}");
    }
    assert_eq!(
        operations(&ast, "DDS::CacheListener"),
        [
            "void on_begin_updates()",
            "void on_end_updates()",
            "void on_updates_enabled()",
            "void on_updates_disabled()",
        ]
    );
    assert_eq!(bases(&ast, "DDS::CacheAccess"), ["DDS::CacheBase"]);
    for exception in ["DDS::DCPSError", "DDS::PreconditionNotMet"] {
        assert_eq!(exception_members(&ast, exception), ["string message"]);
    }
}

/// Annex B, "All the retained functions mimic exactly the DDS::Cache ones
/// [...]. Similarly, all the retained attributes are identical to the
/// DDS::Cache ones" (§8.2.1.1): each attribute and operation of
/// `CacheOperation` is declared once in `DDS::Cache` or its base
/// `DDS::CacheBase`, with the same type, parameters (direction, type,
/// name), result and exceptions. Left out are the set-up of the Cache that
/// the connector fragment does (§8.2.2), the kind of the Cache, and `load`,
/// `lock` and `unlock`.
#[test]
fn dds4ccm_annex_b_cache_operation_retains_attributes_and_operations_of_dds_cache_unchanged() {
    let ast = model_with_dlrl("#include \"ccm_dds_dlrl.idl\"\n");
    let cache = declarations_with_bases(&ast, "DDS::Cache");
    let retained = declarations(&ast, "CCM_DDS::CacheOperation");
    assert_eq!(retained.len(), 13);
    for (name, declaration) in &retained {
        let of_cache: Vec<&String> = cache
            .iter()
            .filter(|(of, _)| of == name)
            .map(|(_, declaration)| declaration)
            .collect();
        assert_eq!(
            of_cache,
            [declaration],
            "{name} of CacheOperation against DDS::Cache"
        );
    }
    let mut left_out: Vec<&str> = cache
        .iter()
        .map(|(name, _)| name.as_str())
        .filter(|name| !retained.iter().any(|(kept, _)| kept == name))
        .collect();
    left_out.sort_unstable();
    assert_eq!(
        left_out,
        [
            "enable_all_for_pubsub",
            "find_home_by_index",
            "find_home_by_name",
            "kind",
            "load",
            "lock",
            "pubsub_state",
            "register_all_for_pubsub",
            "register_home",
            "the_publisher",
            "the_subscriber",
            "unlock",
        ]
    );
}

/// Annex A and Annex B in one unit, in either order: module `CCM_DDS` has
/// the interfaces of both, and module `DDS` declares each name once.
/// `dds_rtf2_dlrl.idl` makes `dds_rtf2_dcps.idl` the one declaration of the
/// DCPS names the generic DLRL IDL declares in its own `dds_dcps.idl`;
/// without that, the two declarations of `DDS::StringSeq` and `DDS::Entity`
/// collide. Without the generic DLRL IDL on the include path, Annex B does
/// not compile.
#[test]
fn dds4ccm_annex_b_compiles_with_annex_a_in_one_unit() {
    for source in [
        format!("#include \"ccm_dds.idl\"\n#include \"ccm_dds_dlrl.idl\"\n{FOO}"),
        format!("#include \"ccm_dds_dlrl.idl\"\n#include \"ccm_dds.idl\"\n{FOO}"),
    ] {
        let ast = model_with_dlrl(&source);
        for local in [
            "CCM_DDS::CacheOperation",
            "CCM_DDS::DataListenerControl",
            "Foo_conn::Writer",
        ] {
            assert_eq!(
                interface(&ast, local).1.kind,
                InterfaceKind::Local,
                "{local}"
            );
        }
        for name in [
            "DDS::StringSeq",
            "DDS::Entity",
            "DDS::DomainParticipant",
            "DDS::Publisher",
            "DDS::Subscriber",
            "DDS::CacheUsage",
        ] {
            let found = ast.find_by_scoped_name(&path(name));
            assert!(found.is_ok(), "{name}: {found:?}");
        }
        let cache = exports(&ast, "DDS::Cache");
        for attribute in [
            "readonly attribute DDS::Publisher the_publisher",
            "readonly attribute DDS::Subscriber the_subscriber",
        ] {
            assert!(
                cache.iter().any(|a| a == attribute),
                "{attribute}: {cache:?}"
            );
        }
    }
    let (_, diagnostics) = compose_beside_the_annexes(
        "#include \"dds_rtf2_dcps.idl\"\n#include \"dds_dcps.idl\"\n",
        &[DLRL_IDL],
    );
    assert!(
        !diagnostics.is_empty(),
        "two declarations of DDS::StringSeq and DDS::Entity compile"
    );
    let (_, diagnostics) = compose_beside_the_annexes("#include \"ccm_dds_dlrl.idl\"\n", &[]);
    assert!(
        diagnostics.iter().any(|d| d.contains("dds_dlrl.idl")),
        "Annex B compiles without the generic DLRL IDL: {diagnostics:#?}"
    );
}

/// The Rust spelling of a result or an attribute value of Annex B.
fn rust_value_b(idl: &str) -> &'static str {
    match idl {
        "void" => "()",
        "boolean" => "bool",
        "DDS::CacheUsage" => "CacheUsage",
        "DDS::CacheAccess" => "CacheAccess",
        "DDS::ObjectRootSeq" => "Vec<ObjectRoot>",
        "DDS::ObjectHomeSeq" => "Vec<ObjectHome>",
        "DDS::CacheAccessSeq" => "Vec<CacheAccess>",
        "DDS::CacheListenerSeq" => "Vec<Arc<dyn CacheListener>>",
        other => panic!("no Rust spelling of a value {other}"),
    }
}

/// The Rust spelling of an `in` parameter of Annex B: an enumeration by
/// value, a CacheAccess borrowed, a CacheListener as the `Arc` that
/// `attach_listener` keeps and `detach_listener` borrows (it finds the
/// listener by identity).
fn rust_in_b(operation: &str, idl: &str) -> &'static str {
    match (operation, idl) {
        (_, "DDS::CacheUsage") => "CacheUsage",
        (_, "DDS::CacheAccess") => "&CacheAccess",
        ("attach_listener", "DDS::CacheListener") => "Arc<dyn CacheListener>",
        ("detach_listener", "DDS::CacheListener") => "&Arc<dyn CacheListener>",
        other => panic!("no Rust spelling of an `in` {other:?}"),
    }
}

/// The methods `CCM_DDS::CacheOperation` maps to in Rust (documentation of
/// the trait `CacheOperation`): an attribute is a getter of its value, an
/// operation a method with the parameters of the IDL that returns `Result`
/// with `DlrlException`, since `AlreadyDeleted` may come from every
/// operation (DDS 1.2 §8.2.1.1).
fn cache_operation_signatures(ast: &Ast) -> Vec<Signature> {
    let (_, node) = interface(ast, "CCM_DDS::CacheOperation");
    let mut methods = Vec::new();
    for id in &node.operations {
        let op = ast.operation(*id).unwrap();
        let params = op
            .params
            .iter()
            .map(|id| {
                let param = ast.param(*id).unwrap();
                assert!(matches!(param.direction, ParamDirection::In));
                let spelled = rust_in_b(&op.name, &type_name(ast, &param.type_));
                (param.name.clone(), spelled.to_string())
            })
            .collect();
        let value = op
            .return_type
            .as_ref()
            .map_or("()", |r| rust_value_b(&type_name(ast, r)));
        methods.push(Signature {
            name: op.name.clone(),
            params,
            result: format!("Result<{value}, DlrlException>"),
        });
    }
    for id in &node.attributes {
        let attribute = ast.attribute(*id).unwrap();
        assert!(attribute.readonly && attribute.get_raises.is_empty());
        methods.push(Signature {
            name: attribute.name.clone(),
            params: Vec::new(),
            result: rust_value_b(&type_name(ast, &attribute.type_)).to_string(),
        });
    }
    methods.sort();
    methods
}

/// The methods `names` of the `impl {type_} {` blocks of a Rust source
/// text, with their parameters and results.
fn inherent_signatures(source: &str, type_: &str, names: &[&str]) -> Vec<Signature> {
    let head = format!("impl {type_} {{");
    let mut declarations = Vec::new();
    let (mut inside, mut current): (bool, Option<String>) = (false, None);
    for line in source.lines() {
        if line == head {
            inside = true;
            continue;
        }
        if !inside {
            continue;
        }
        if line == "}" {
            inside = false;
            continue;
        }
        if let Some(text) = current.as_mut() {
            text.push(' ');
            text.push_str(line.trim());
        } else if let Some(rest) = line.strip_prefix("    pub fn ") {
            current = Some(rest.to_string());
        } else {
            continue;
        }
        if current.as_deref().is_some_and(|text| text.ends_with('{')) {
            let text = current.take().unwrap_or_default();
            declarations.push(text.trim_end_matches('{').trim().to_string());
        }
    }
    declarations
        .iter()
        .filter(|declaration| {
            let name = declaration.split('(').next().unwrap_or_default().trim();
            names.contains(&name)
        })
        .map(|declaration| signature_of(declaration, type_))
        .collect()
}

/// Annex B against the Rust interface: the trait `CacheOperation` has the
/// attributes and operations of the IDL and no other, each with the
/// parameters of the IDL (name, order, type) and its result; each method
/// has the signature of the method of `zerodds_dlrl::cache::Cache` of the
/// same name ("mimic exactly the DDS::Cache ones"); the exceptions of the
/// `raises` clauses, and `AlreadyDeleted`, are variants of `DlrlException`
/// with the repository id of the exception of the DLRL IDL and its
/// `message`.
#[test]
fn dds4ccm_annex_b_the_rust_cache_operation_has_the_operations_of_the_idl() {
    use zerodds_dds4ccm::dlrl::{Cache, CacheOperation, DlrlException};

    let ast = model_with_dlrl("#include \"ccm_dds_dlrl.idl\"\n");
    let expected = cache_operation_signatures(&ast);
    let mut of_trait = trait_signatures(include_str!("../src/dlrl.rs"), "CacheOperation");
    of_trait.sort();
    assert_eq!(of_trait, expected, "trait CacheOperation against the IDL");
    let names: Vec<&str> = expected.iter().map(|m| m.name.as_str()).collect();
    // The source of zerodds-dlrl beside this crate in the workspace.
    let cache_source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../dlrl/src/cache/mod.rs"
    ))
    .expect("the source of zerodds_dlrl::cache");
    let mut of_cache = inherent_signatures(&cache_source, "Cache", &names);
    of_cache.sort();
    assert_eq!(of_cache, expected, "the methods of Cache the trait mimics");

    assert_eq!(
        raised_by(&ast, "CCM_DDS::CacheOperation"),
        ["DDS::DCPSError", "DDS::PreconditionNotMet"]
    );
    for (idl, rust) in [
        ("DDS::DCPSError", DlrlException::dcps("m")),
        ("DDS::PreconditionNotMet", DlrlException::precondition("m")),
        ("DDS::AlreadyDeleted", DlrlException::already_deleted("m")),
    ] {
        assert_eq!(
            rust.repository_id(),
            format!("IDL:{}:1.0", idl.replace("::", "/"))
        );
        assert_eq!(exception_members(&ast, idl), ["string message"]);
        assert_eq!(rust.message(), "m");
    }

    // The trait is the one the comparison read, and Cache implements it.
    let _ = <dyn CacheOperation>::create_access;
    let _ = <dyn CacheOperation>::listeners;
    fn implements<T: CacheOperation>() {}
    implements::<Cache>();
}
