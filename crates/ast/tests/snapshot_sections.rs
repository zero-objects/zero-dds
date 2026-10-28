// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Snapshot round-trip, one node kind at a time.
//!
//! A whole-model test tells you the file is wrong; these tell you which
//! section is, because each starts from the smallest model that reaches
//! one kind.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::{Ast, MemberSpec, PrimitiveKind, TypeKind};
use zerodds_ast_core::Ref;

fn round_trips(ast: &Ast, what: &str) {
    let bytes = ast.to_snapshot().expect("snapshot");
    match Ast::from_snapshot(&bytes) {
        Ok(loaded) => {
            assert_eq!(
                loaded.node_count(),
                ast.node_count(),
                "{what}: node count changed"
            );
            assert_eq!(
                loaded.to_snapshot().expect("re-snapshot"),
                bytes,
                "{what}: not byte-identical"
            );
        }
        Err(error) => panic!("{what}: {error}"),
    }
}

#[test]
fn an_empty_model_round_trips() {
    round_trips(&Ast::new(), "empty model");
}

#[test]
fn a_module_round_trips() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let _ = ast.ensure_module(root, "Robot").expect("Robot");
    round_trips(&ast, "module");
}

#[test]
fn a_type_round_trips() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let _ = ast
        .add_type(root, "L", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("L");
    round_trips(&ast, "type");
}

#[test]
fn an_expression_round_trips() {
    let mut ast = Ast::new();
    let _ = ast.add_int_expr(7).expect("7");
    round_trips(&ast, "expression");
}

#[test]
fn a_struct_member_round_trips() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "L", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("L");
    let s = ast
        .add_type(
            root,
            "S",
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("S");
    let _ = ast
        .add_member(s, MemberSpec::new("x", Ref::Resolved(long)))
        .expect("x");
    round_trips(&ast, "struct member");
}

#[test]
fn a_constant_round_trips() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "L", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("L");
    let expr = ast.add_int_expr(7).expect("7");
    let _ = ast
        .add_const(root, "MAX", Ref::Resolved(long), expr)
        .expect("MAX");
    round_trips(&ast, "constant");
    // The type, which no snapshot before version 3 wrote: every constant
    // came back typed `Broken(NotFound)` with an empty name.
    let loaded = Ast::from_snapshot(&ast.to_snapshot().expect("snapshot")).expect("load");
    let (_, constant) = loaded.constants().next().expect("MAX");
    assert_eq!(
        constant.type_,
        Ref::Resolved(long),
        "the constant lost its type"
    );
}

/// A reference the model has not resolved yet comes back waiting on the
/// same key, and the declaration that arrives after the load resolves it —
/// the waiter index is rebuilt with the model.
#[test]
fn a_pending_reference_comes_back_pending_on_the_same_key() {
    use zerodds_ast_core::{PendingRef, ScopedName};
    let key = PendingRef::ScopedName(ScopedName::absolute(vec!["Later".into()]));
    let mut ast = Ast::new();
    let root = ast.root_module();
    let holder = ast
        .add_type(
            root,
            "Holder",
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("Holder");
    let member = ast
        .add_member(holder, MemberSpec::new("x", Ref::Pending(key.clone())))
        .expect("x");
    round_trips(&ast, "pending reference");

    let mut loaded = Ast::from_snapshot(&ast.to_snapshot().expect("snapshot")).expect("load");
    assert_eq!(
        loaded.member(member).expect("x").type_,
        Ref::Pending(key),
        "the reference did not come back on its key"
    );
    let later = loaded
        .add_type(root, "Later", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("Later");
    let _ = loaded.resolve_all_by_name().expect("resolve");
    assert_eq!(
        loaded.member(member).expect("x").type_,
        Ref::Resolved(later),
        "the loaded model does not resolve what it was waiting on"
    );
}

/// A broken reference keeps its cause, the name it looked for included.
#[test]
fn a_broken_reference_keeps_its_cause() {
    use zerodds_ast_core::{BreakCause, PendingRef, ScopedName};
    let key = PendingRef::ScopedName(ScopedName::relative(vec!["Robot".into(), "Gone".into()]));
    let mut ast = Ast::new();
    let root = ast.root_module();
    let holder = ast
        .add_type(
            root,
            "Holder",
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("Holder");
    let member = ast
        .add_member(holder, MemberSpec::new("x", Ref::Pending(key)))
        .expect("x");
    let _ = ast.resolve_all_by_name().expect("resolve");
    let broken = ast.member(member).expect("x").type_.clone();
    assert!(
        matches!(&broken, Ref::Broken(BreakCause::NotFound { .. })),
        "{broken:?}"
    );
    round_trips(&ast, "broken reference");
    let loaded = Ast::from_snapshot(&ast.to_snapshot().expect("snapshot")).expect("load");
    assert_eq!(loaded.member(member).expect("x").type_, broken);
}

#[test]
fn an_exception_and_its_member_round_trip() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "L", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("L");
    let ex = ast.add_exception(root, "Failure").expect("Failure");
    let _ = ast
        .add_exception_member(ex, MemberSpec::new("code", Ref::Resolved(long)))
        .expect("code");
    round_trips(&ast, "exception");
}

#[test]
fn an_interface_with_an_operation_round_trips() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "L", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("L");
    let iface = ast
        .add_interface(root, "C", zerodds_ast::InterfaceKind::Local, Vec::new())
        .expect("C");
    let op = ast
        .add_operation(iface, "go", Some(Ref::Resolved(long)), true)
        .expect("go");
    let _ = ast
        .add_param(
            op,
            "d",
            zerodds_ast::ParamDirection::InOut,
            Ref::Resolved(long),
        )
        .expect("d");
    let _ = ast
        .add_attribute(iface, "speed", Ref::Resolved(long), true)
        .expect("speed");
    round_trips(&ast, "interface");
}

#[test]
fn a_union_case_round_trips() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "L", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("L");
    let u = ast
        .add_type(
            root,
            "U",
            TypeKind::Union {
                discriminator: Ref::Resolved(long),
                cases: Vec::new(),
                default_case: None,
            },
        )
        .expect("U");
    let _ = ast
        .add_case(
            u,
            vec![zerodds_ast::CaseLabel::Default],
            MemberSpec::new("v", Ref::Resolved(long)),
        )
        .expect("case");
    round_trips(&ast, "union case");
}

#[test]
fn an_enumerator_a_bit_and_a_bitfield_round_trip() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let e = ast
        .add_type(
            root,
            "E",
            TypeKind::Enumeration {
                enumerators: Vec::new(),
            },
        )
        .expect("E");
    let _ = ast.add_enumerator(e, "RED", None).expect("RED");
    let b = ast
        .add_type(root, "B", TypeKind::Bitmask { bits: Vec::new() })
        .expect("B");
    let _ = ast.add_bit_value(b, "A", Some(3)).expect("A");
    let bs = ast
        .add_type(
            root,
            "BS",
            TypeKind::Bitset {
                base: None,
                fields: Vec::new(),
            },
        )
        .expect("BS");
    let _ = ast.add_bitfield(bs, Some("f"), 2, None).expect("f");
    round_trips(&ast, "enumerator, bit, bitfield");
}

#[test]
fn an_annotation_declaration_round_trips() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "L", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("L");
    let ann = ast.add_annotation_def(root, "my_ann").expect("@my_ann");
    let _ = ast
        .add_annotation_member(ann, "value", Ref::Resolved(long), None)
        .expect("value");
    round_trips(&ast, "annotation");
}

#[test]
fn the_deployment_layer_round_trips() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let s = ast
        .add_type(
            root,
            "Pose",
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("Pose");
    let lib = ast.add_library("Deployment").expect("library");
    let profile = ast
        .add_qos_profile(
            lib,
            "R",
            None,
            vec![zerodds_ast::QosPolicy::Reliability {
                kind: None,
                max_blocking_time: None,
            }],
        )
        .expect("profile");
    let domain = ast.add_domain(lib, "Main", 3).expect("domain");
    let topic = ast
        .add_topic(domain, "T", Ref::Resolved(s), Some(Ref::Resolved(profile)))
        .expect("topic");
    let participant = ast
        .add_participant(lib, "P", Ref::Resolved(domain), None)
        .expect("participant");
    let _ = ast
        .add_endpoint(
            participant,
            "w",
            zerodds_ast::EndpointRole::DataWriter,
            Ref::Resolved(topic),
            None,
        )
        .expect("writer");
    round_trips(&ast, "deployment");
}

// -- facets, field by field ----------------------------------------------

/// Compare the facets of one node before and after the round trip.
///
/// The byte-identity check above cannot see a field that neither the
/// writer nor the reader touches: it writes nothing and reads nothing,
/// so the re-snapshot matches while the value is gone. This compares
/// the values themselves.
fn facets_survive(ast: &Ast, any: zerodds_ast::AnyId, what: &str) {
    let bytes = ast.to_snapshot().expect("snapshot");
    let loaded = Ast::from_snapshot(&bytes).expect("load");
    let node_id = any.node_id();
    let restored = loaded
        .resolve_node_id(node_id)
        .unwrap_or_else(|| panic!("{what}: the node itself is gone"));
    assert_eq!(
        loaded.facets(restored).expect("loaded facets"),
        ast.facets(any).expect("original facets"),
        "{what}: a facet field did not survive the round trip"
    );
}

#[test]
fn every_xtypes_field_survives_the_round_trip() {
    use zerodds_ast::{
        AutoId, DataRepresentationMask, Extensibility, MemberIdAssignment, MemberIdOrigin,
        RangeConstraint, TopicAnnotation, TryConstruct, VerbatimDirective, VerbatimPlacement,
    };

    let mut ast = Ast::new();
    let root = ast.root_module();
    let s = ast
        .add_type(
            root,
            "S",
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("S");
    let min = ast.add_int_expr(0).expect("min");
    let max = ast.add_int_expr(100).expect("max");
    let default = ast.add_int_expr(7).expect("default");

    let any = ast.any_of_type(s).expect("any");
    let facets = ast.facets_mut(any).expect("facets");
    let x = facets.xtypes_mut();
    x.extensibility = Some(Extensibility::Mutable);
    x.is_key = true;
    x.is_optional = true;
    x.is_external = true;
    x.is_must_understand = true;
    x.is_non_serialized = true;
    x.is_nested = true;
    x.is_default_literal = true;
    x.try_construct = Some(TryConstruct::UseDefault);
    x.member_id = Some(MemberIdAssignment {
        effective: 11,
        origin: MemberIdOrigin::Hashed {
            source_name: "written".to_string(),
        },
    });
    x.bit_bound = Some(9);
    x.default_value = Some(default);
    x.range = Some(RangeConstraint {
        min: Some(min),
        max: Some(max),
    });
    x.unit = Some("m/s".to_string());
    x.verbatim.push(VerbatimDirective {
        language: "c++".to_string(),
        placement: VerbatimPlacement::EndFile,
        text: "// tail".to_string(),
    });
    x.autoid = Some(AutoId::Hash);
    x.topic = Some(TopicAnnotation {
        name: Some("Shapes".to_string()),
        platform: Some("*".to_string()),
    });
    x.data_representation = Some(DataRepresentationMask {
        xcdr1: true,
        xcdr2: true,
        xml: false,
    });
    x.ignore_literal_names = true;

    facets_survive(&ast, any, "xtypes");
}

#[test]
fn every_corba_field_survives_the_round_trip() {
    use zerodds_ast::facet::corba::TemplateArg;
    use zerodds_ast::{ComponentRole, RepositoryIdOrigin, TemplateParam, TemplateParamKind};

    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "long", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("long");
    let event = ast
        .add_type(
            root,
            "Event",
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("Event");
    let expr = ast.add_int_expr(3).expect("expr");
    let s = ast
        .add_type(
            root,
            "S",
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("S");

    let any = ast.any_of_type(s).expect("any");
    let facets = ast.facets_mut(any).expect("facets");
    let c = facets.corba_mut();
    c.repository_id = Some(zerodds_ast_core::RepositoryId("IDL:S:1.0".to_string()));
    c.repository_id_origin = Some(RepositoryIdOrigin::PragmaPrefix);
    c.component_role = Some(ComponentRole::EventSink {
        name: "sink".to_string(),
        event: Ref::Resolved(event),
    });
    c.template_params.push(TemplateParam {
        name: "T".to_string(),
        kind: TemplateParamKind::Const,
        constraint: Some(Ref::Resolved(long)),
    });
    c.template_args.push(TemplateArg::Type(Ref::Resolved(long)));
    c.template_args.push(TemplateArg::Const(expr));
    c.native_mapping = Some("void*".to_string());
    c.is_service = true;
    c.is_ami = true;
    c.imports = vec!["IDL:Other:1.0".to_string()];

    facets_survive(&ast, any, "corba");
}

#[test]
fn every_ros2_and_xml_field_survives_the_round_trip() {
    use zerodds_ast::{RosBoundedOrigin, RosInterfaceRole};

    let mut ast = Ast::new();
    let root = ast.root_module();
    let s = ast
        .add_type(
            root,
            "S",
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("S");
    let default = ast.add_int_expr(1).expect("default");

    let any = ast.any_of_type(s).expect("any");
    let facets = ast.facets_mut(any).expect("facets");
    let ros = facets.ros2_mut();
    ros.default_value = Some(default);
    ros.bounded_origin = Some(RosBoundedOrigin::BoundedString);
    ros.interface_role = Some(RosInterfaceRole::Feedback);
    ros.ros_name = Some(zerodds_ast_core::RosTypeName {
        package: "geometry_msgs".to_string(),
        kind: zerodds_ast_core::RosInterfaceKind::Msg,
        name: "Pose".to_string(),
    });
    ros.was_constant = true;

    let facets = ast.facets_mut(any).expect("facets");
    let xml = facets.xml_mut();
    xml.element = "struct".to_string();
    xml.attributes.push(("name".to_string(), "S".to_string()));
    xml.unresolved_idrefs
        .push(zerodds_ast_core::XmlRefKey("ref-1".to_string()));
    xml.xsd_type = Some("structType".to_string());
    xml.library_path.push("lib".to_string());

    facets_survive(&ast, any, "ros2 and xml");
}

#[test]
fn every_vendor_directive_survives_the_round_trip() {
    // Vendor directives are never normalised into core constructs, so
    // dropping them on the way through a snapshot loses exactly what a
    // round trip back to that dialect needs. Spec §4.6.
    use zerodds_ast::{CoreDxFacet, FastDdsFacet, OpenDdsFacet, RtiFacet};

    let mut ast = Ast::new();
    let root = ast.root_module();
    let s = ast
        .add_type(
            root,
            "S",
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("S");

    let any = ast.any_of_type(s).expect("any");
    let facets = ast.facets_mut(any).expect("facets");
    facets.vendor.rti = Some(RtiFacet {
        copy_directives: vec!["// copied".to_string()],
        resolve_name: Some(false),
        top_level: Some(true),
        keylist: vec!["vin".to_string()],
        unrecognised: vec![("odd".to_string(), "value".to_string())],
    });
    facets.vendor.open_dds = Some(OpenDdsFacet {
        dcps_data_type: true,
        dcps_data_keys: vec!["id".to_string()],
        supports_zero_copy: Some(true),
        unrecognised: Vec::new(),
    });
    facets.vendor.fast_dds = Some(FastDdsFacet {
        xtypes_extensions: vec!["ext".to_string()],
        plain_data_type: Some(false),
        unrecognised: Vec::new(),
    });
    facets.vendor.core_dx = Some(CoreDxFacet {
        key_fields: vec!["k".to_string()],
        unrecognised: vec![("x".to_string(), "y".to_string())],
    });

    facets_survive(&ast, any, "vendor");
}

#[test]
fn every_literal_form_survives_the_round_trip() {
    // Everything but integers and booleans used to be written as one
    // tag with no payload and read back as `FALSE`: a string constant
    // came out of a snapshot as a boolean, and the byte comparison
    // above could not see it because the second write agreed with the
    // first read.
    use zerodds_ast::{ConstExpr, Literal};

    let mut ast = Ast::new();
    let literals = vec![
        Literal::Integer(-7),
        Literal::Boolean(true),
        Literal::Float {
            mantissa: "1.".to_string(),
            exponent: None,
        },
        Literal::Float {
            mantissa: "6.02".to_string(),
            exponent: Some("+23".to_string()),
        },
        Literal::Fixed {
            digits: "12345".to_string(),
            scale: 2,
        },
        Literal::Char('ä'),
        Literal::WChar(0x1F600),
        Literal::String("a\nb".to_string()),
        Literal::WString(vec![0x41, 0x1F600]),
    ];

    let mut ids = Vec::new();
    for literal in &literals {
        ids.push(
            ast.add_expr(ConstExpr::Literal(literal.clone()))
                .expect("expression"),
        );
    }

    let bytes = ast.to_snapshot().expect("snapshot");
    let loaded = Ast::from_snapshot(&bytes).expect("load");

    for (id, expected) in ids.iter().zip(literals.iter()) {
        let node_id = ast.expr(*id).expect("expression").node_id;
        let any = loaded.resolve_node_id(node_id).expect("expression");
        let zerodds_ast::AnyId::ConstExpr(restored, _) = any else {
            panic!("expected an expression, got {any:?}");
        };
        assert_eq!(
            loaded.expr(restored).expect("expression").expr,
            ConstExpr::Literal(expected.clone()),
            "{expected:?} did not survive"
        );
    }
}

#[test]
fn an_expression_naming_a_constant_survives_as_a_name_reference() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "long", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("long");
    let value = ast.add_int_expr(3).expect("value");
    let constant = ast
        .add_const(root, "N", Ref::Resolved(long), value)
        .expect("N");
    let reference = ast
        .add_expr(zerodds_ast::ConstExpr::NameRef(Ref::Resolved(constant)))
        .expect("reference");

    let node_id = ast.expr(reference).expect("reference").node_id;
    let bytes = ast.to_snapshot().expect("snapshot");
    let loaded = Ast::from_snapshot(&bytes).expect("load");

    let any = loaded.resolve_node_id(node_id).expect("reference");
    let zerodds_ast::AnyId::ConstExpr(restored, _) = any else {
        panic!("expected an expression");
    };
    let zerodds_ast::ConstExpr::NameRef(restored) = &loaded.expr(restored).expect("expr").expr
    else {
        panic!("the name reference became something else");
    };
    assert!(restored.is_resolved(), "the reference lost its target");
}

#[test]
fn every_type_kind_comes_back_as_the_kind_it_went_in_as() {
    // Six kinds were written as `tag::NATIVE`: a valuetype loaded as a
    // `native` with no state, no bases, no initialisers and no supported
    // interfaces, and so did a boxed value, an object reference, `any`,
    // `ValueBase` and an annotation type. The round-trip test could not
    // see it, because the model it built had none of the six in it.
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "long", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("long");
    let interface = ast
        .add_interface(root, "I", zerodds_ast::InterfaceKind::Interface, Vec::new())
        .expect("I");
    let definition = ast.add_annotation_def(root, "ann").expect("ann");

    let kinds = [
        (
            "valuetype",
            TypeKind::Valuetype {
                kind: zerodds_ast::ValuetypeKind::Truncatable,
                bases: Vec::new(),
                state: Vec::new(),
                initializers: Vec::new(),
                supported: vec![Ref::Resolved(interface)],
            },
        ),
        (
            "value box",
            TypeKind::ValueBox {
                boxed: Ref::Resolved(long),
            },
        ),
        (
            "object reference",
            TypeKind::ObjectRef {
                interface: Some(Ref::Resolved(interface)),
            },
        ),
        (
            "generic object reference",
            TypeKind::ObjectRef { interface: None },
        ),
        ("any", TypeKind::Any),
        ("ValueBase", TypeKind::ValueBase),
        ("annotation type", TypeKind::AnnotationType { definition }),
        ("native", TypeKind::Native),
    ];

    for (index, (what, kind)) in kinds.into_iter().enumerate() {
        let name = format!("T{index}");
        let id = ast.add_type(root, &name, kind.clone()).expect("declare");
        let node_id = ast.type_(id).expect("declared").node_id;
        let bytes = ast.to_snapshot().expect("snapshot");
        let loaded = Ast::from_snapshot(&bytes).expect("load");
        let zerodds_ast::AnyId::Type(restored, _) =
            loaded.resolve_node_id(node_id).expect("the type")
        else {
            panic!("{what}: came back as something other than a type");
        };
        assert_eq!(
            loaded.type_(restored).expect("restored").kind,
            kind,
            "{what}: did not survive the round trip"
        );
    }
}

#[test]
fn a_union_keeps_which_case_is_the_default_one() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "long", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("long");
    let union = ast
        .add_type(
            root,
            "U",
            TypeKind::Union {
                discriminator: Ref::Resolved(long),
                cases: Vec::new(),
                default_case: None,
            },
        )
        .expect("U");
    let case = ast
        .add_case(
            union,
            vec![zerodds_ast::CaseLabel::Default],
            MemberSpec::new("x", Ref::Resolved(long)),
        )
        .expect("default case");
    let node_id = ast.type_(union).expect("U").node_id;

    let bytes = ast.to_snapshot().expect("snapshot");
    let loaded = Ast::from_snapshot(&bytes).expect("load");
    let zerodds_ast::AnyId::Type(restored, _) = loaded.resolve_node_id(node_id).expect("U") else {
        panic!("U came back as something other than a type");
    };
    let TypeKind::Union { default_case, .. } = &loaded.type_(restored).expect("U").kind else {
        panic!("U came back as something other than a union");
    };
    assert_eq!(*default_case, Some(case), "the default case was dropped");
}

#[test]
fn a_bitset_keeps_the_bitset_it_inherits_from() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let base = ast
        .add_type(
            root,
            "Base",
            TypeKind::Bitset {
                base: None,
                fields: Vec::new(),
            },
        )
        .expect("Base");
    let derived = ast
        .add_type(
            root,
            "Derived",
            TypeKind::Bitset {
                base: Some(Ref::Resolved(base)),
                fields: Vec::new(),
            },
        )
        .expect("Derived");
    let node_id = ast.type_(derived).expect("Derived").node_id;

    let bytes = ast.to_snapshot().expect("snapshot");
    let loaded = Ast::from_snapshot(&bytes).expect("load");
    let zerodds_ast::AnyId::Type(restored, _) = loaded.resolve_node_id(node_id).expect("Derived")
    else {
        panic!("Derived came back as something other than a type");
    };
    let TypeKind::Bitset { base, .. } = &loaded.type_(restored).expect("Derived").kind else {
        panic!("Derived came back as something other than a bitset");
    };
    assert!(base.is_some(), "the inherited bitset was dropped");
}

#[test]
fn the_corba_facet_keeps_a_supports_clause_and_a_state_visibility() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let interface = ast
        .add_interface(root, "I", zerodds_ast::InterfaceKind::Interface, Vec::new())
        .expect("I");
    let component = ast
        .add_interface(root, "C", zerodds_ast::InterfaceKind::Component, Vec::new())
        .expect("C");
    let node_id = ast.interface(component).expect("C").node_id;
    if let Some(facets) = ast.facets_mut(zerodds_ast::AnyId::Interface(component, node_id)) {
        facets.corba_mut().supports = vec![Ref::Resolved(interface)];
        facets.corba_mut().state_visibility = Some(zerodds_ast::StateVisibility::Private);
    }

    let bytes = ast.to_snapshot().expect("snapshot");
    let loaded = Ast::from_snapshot(&bytes).expect("load");
    let any = loaded.resolve_node_id(node_id).expect("C");
    let corba = loaded
        .facets(any)
        .and_then(|facets| facets.corba.as_ref())
        .expect("the CORBA facet");
    assert_eq!(corba.supports.len(), 1, "the supports clause was dropped");
    assert_eq!(
        corba.state_visibility,
        Some(zerodds_ast::StateVisibility::Private),
        "the visibility keyword was dropped"
    );
}

/// The interface or value type a declaration sits in survives the
/// snapshot, and the loaded model names the declaration through it.
#[test]
fn a_declaration_scope_round_trips() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let module = ast.ensure_module(root, "CosNaming").expect("CosNaming");
    let ex = ast.add_exception(module, "NotFound").expect("NotFound");
    let ty = ast
        .add_type(module, "Count", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("Count");
    let ex_node = ast.exception(ex).expect("exception").node_id;
    let ty_node = ast.type_(ty).expect("type").node_id;
    for any in [
        zerodds_ast::AnyId::Exception(ex, ex_node),
        zerodds_ast::AnyId::Type(ty, ty_node),
    ] {
        ast.set_declaration_scope(any, "NamingContext")
            .expect("scope");
    }
    round_trips(&ast, "declaration scope");
    let loaded = Ast::from_snapshot(&ast.to_snapshot().expect("snapshot")).expect("load");
    for name in ["NotFound", "Count"] {
        let scoped = zerodds_ast_core::ScopedName::absolute(
            ["CosNaming", "NamingContext", name]
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
        );
        assert!(loaded.find_by_scoped_name(&scoped).is_ok(), "{name}");
    }
}
