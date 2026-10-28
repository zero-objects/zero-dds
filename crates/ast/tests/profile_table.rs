// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The profile construct table is total. Spec §8.2.
//!
//! Completeness here means something checkable: every construct the
//! model can hold, and every facet property a profile can forbid, has an
//! explicit ruling in every profile. The table itself has no catch-all
//! arm, so an unruled pair is a compile error — these tests prove the
//! rulings are also *reachable* from a real model, which the compiler
//! cannot.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::facet::xtypes::{MemberIdAssignment, MemberIdOrigin};
use zerodds_ast::profile::ALL_PROFILES;
use zerodds_ast::{
    Ast, Bound, CharWidth, ConstructKind, Extensibility, FacetProperty, MemberSpec, PrimitiveKind,
    Profile, TypeKind, ValuetypeKind, Verdict, construct_verdict, facet_verdict,
};
use zerodds_ast_core::{Id, Ref};

/// One instance of every construct the model can hold.
///
/// If a construct cannot be built here, it cannot be reached by a
/// frontend either, and its ruling would be dead.
fn model_with_every_construct() -> (Ast, Vec<(ConstructKind, Id<zerodds_ast::TypeNode>)>) {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let mut out = Vec::new();

    let long = ast
        .add_type(root, "L", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("L");
    out.push((ConstructKind::Primitive, long));

    let narrow = ast
        .add_type(root, "I8", TypeKind::Primitive(PrimitiveKind::Int8))
        .expect("I8");
    out.push((ConstructKind::NarrowInteger, narrow));

    let long_double = ast
        .add_type(root, "LD", TypeKind::Primitive(PrimitiveKind::Float128))
        .expect("LD");
    out.push((ConstructKind::LongDouble, long_double));

    let fixed = ast
        .add_type(
            root,
            "Money",
            TypeKind::Fixed {
                digits: 9,
                scale: 2,
            },
        )
        .expect("Money");
    out.push((ConstructKind::Fixed, fixed));

    let text = ast
        .add_type(
            root,
            "S",
            TypeKind::String {
                width: CharWidth::Char,
                bound: Bound::Unbounded,
            },
        )
        .expect("S");
    out.push((ConstructKind::String, text));

    let wide = ast
        .add_type(
            root,
            "WS",
            TypeKind::String {
                width: CharWidth::WChar,
                bound: Bound::Unbounded,
            },
        )
        .expect("WS");
    out.push((ConstructKind::WideString, wide));

    let seq = ast
        .add_type(
            root,
            "LSeq",
            TypeKind::Sequence {
                element: Ref::Resolved(long),
                bound: Bound::Unbounded,
            },
        )
        .expect("LSeq");
    out.push((ConstructKind::Sequence, seq));

    let one = ast.add_int_expr(4).expect("4");
    let two = ast.add_int_expr(2).expect("2");
    let array1 = ast
        .add_type(
            root,
            "Quad",
            TypeKind::Array {
                element: Ref::Resolved(long),
                dims: vec![one],
            },
        )
        .expect("Quad");
    out.push((ConstructKind::Array1D, array1));

    let array2 = ast
        .add_type(
            root,
            "Grid",
            TypeKind::Array {
                element: Ref::Resolved(long),
                dims: vec![one, two],
            },
        )
        .expect("Grid");
    out.push((ConstructKind::ArrayMultiD, array2));

    let map = ast
        .add_type(
            root,
            "Lookup",
            TypeKind::Map {
                key: Ref::Resolved(long),
                value: Ref::Resolved(long),
                bound: Bound::Unbounded,
            },
        )
        .expect("Lookup");
    out.push((ConstructKind::Map, map));

    let structure = ast
        .add_type(
            root,
            "Point",
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("Point");
    out.push((ConstructKind::Structure, structure));

    let union = ast
        .add_type(
            root,
            "Value",
            TypeKind::Union {
                discriminator: Ref::Resolved(long),
                cases: Vec::new(),
                default_case: None,
            },
        )
        .expect("Value");
    out.push((ConstructKind::Union, union));

    let enumeration = ast
        .add_type(
            root,
            "Colour",
            TypeKind::Enumeration {
                enumerators: Vec::new(),
            },
        )
        .expect("Colour");
    out.push((ConstructKind::Enumeration, enumeration));

    let bitmask = ast
        .add_type(root, "Flags", TypeKind::Bitmask { bits: Vec::new() })
        .expect("Flags");
    out.push((ConstructKind::Bitmask, bitmask));

    let bitset = ast
        .add_type(
            root,
            "Header",
            TypeKind::Bitset {
                base: None,
                fields: Vec::new(),
            },
        )
        .expect("Header");
    out.push((ConstructKind::Bitset, bitset));

    let alias = ast
        .add_type(
            root,
            "Name",
            TypeKind::Alias {
                aliased: Ref::Resolved(text),
            },
        )
        .expect("Name");
    out.push((ConstructKind::Alias, alias));

    let valuetype = ast
        .add_type(
            root,
            "V",
            TypeKind::Valuetype {
                kind: ValuetypeKind::Regular,
                bases: Vec::new(),
                state: Vec::new(),
                initializers: Vec::new(),
                supported: Vec::new(),
            },
        )
        .expect("V");
    out.push((ConstructKind::Valuetype, valuetype));

    let value_box = ast
        .add_type(
            root,
            "BoxedL",
            TypeKind::ValueBox {
                boxed: Ref::Resolved(long),
            },
        )
        .expect("BoxedL");
    out.push((ConstructKind::ValueBox, value_box));

    let native = ast
        .add_type(root, "Opaque", TypeKind::Native)
        .expect("Opaque");
    out.push((ConstructKind::Native, native));

    let any = ast.add_type(root, "any", TypeKind::Any).expect("any");
    out.push((ConstructKind::Any, any));

    let value_base = ast
        .add_type(root, "ValueBase", TypeKind::ValueBase)
        .expect("ValueBase");
    out.push((ConstructKind::ValueBase, value_base));

    let iface = ast
        .add_interface(
            root,
            "Control",
            zerodds_ast::InterfaceKind::Interface,
            Vec::new(),
        )
        .expect("Control");
    let object_ref = ast
        .add_type(
            root,
            "ControlRef",
            TypeKind::ObjectRef {
                interface: Some(Ref::Resolved(iface)),
            },
        )
        .expect("ControlRef");
    out.push((ConstructKind::ObjectRef, object_ref));

    let annotation_def = ast.add_annotation_def(root, "my_ann").expect("@my_ann");
    let annotation_type = ast
        .add_type(
            root,
            "MyAnn",
            TypeKind::AnnotationType {
                definition: annotation_def,
            },
        )
        .expect("MyAnn");
    out.push((ConstructKind::AnnotationType, annotation_type));

    (ast, out)
}

#[test]
fn the_construct_table_is_total_over_profiles_and_constructs() {
    // Not a tautology: the table has no catch-all arm, so this asserts
    // the enumeration in ConstructKind::ALL matches the arms the
    // compiler accepted.
    let mut rulings = 0usize;
    for profile in ALL_PROFILES {
        for construct in ConstructKind::ALL {
            let verdict = construct_verdict(profile, construct);
            if let Verdict::Rejected {
                rule,
                spec_reference,
                detail,
            } = verdict
            {
                assert!(!rule.is_empty(), "{profile:?}/{construct:?} rule");
                assert!(
                    !spec_reference.is_empty(),
                    "{profile:?}/{construct:?} paragraph"
                );
                assert!(!detail.is_empty(), "{profile:?}/{construct:?} detail");
            }
            rulings += 1;
        }
    }
    assert_eq!(rulings, ALL_PROFILES.len() * ConstructKind::ALL.len());
    assert_eq!(ConstructKind::ALL.len(), 23);
}

#[test]
fn the_facet_table_is_total_over_profiles_and_properties() {
    let mut rulings = 0usize;
    for profile in ALL_PROFILES {
        for property in FacetProperty::ALL {
            if let Verdict::Rejected {
                rule,
                spec_reference,
                detail,
            } = facet_verdict(profile, property)
            {
                assert!(!rule.is_empty(), "{profile:?}/{property:?} rule");
                assert!(
                    !spec_reference.is_empty(),
                    "{profile:?}/{property:?} paragraph"
                );
                assert!(!detail.is_empty(), "{profile:?}/{property:?} detail");
            }
            rulings += 1;
        }
    }
    assert_eq!(rulings, ALL_PROFILES.len() * FacetProperty::ALL.len());
    assert_eq!(FacetProperty::ALL.len(), 14);
}

#[test]
fn every_construct_the_model_can_hold_is_classified_distinctly() {
    // A construct that classifies as something else would silently
    // inherit that construct's rulings.
    let (ast, constructs) = model_with_every_construct();
    for (expected, id) in &constructs {
        let node = ast.type_(*id).expect("type");
        assert_eq!(
            ConstructKind::of(&node.kind),
            *expected,
            "{} classified wrongly",
            node.name
        );
    }
    assert_eq!(constructs.len(), ConstructKind::ALL.len());
}

#[test]
fn validation_reports_exactly_the_constructs_the_table_rejects() {
    // The link between table and validator: for each profile, the set of
    // types validate() complains about is the set the table rejects.
    let (ast, constructs) = model_with_every_construct();

    for profile in ALL_PROFILES {
        // FlatData rules on reachability rather than on the type itself.
        if profile == Profile::FlatData {
            continue;
        }
        let expected: Vec<ConstructKind> = constructs
            .iter()
            .filter(|(construct, _)| !construct_verdict(profile, *construct).is_allowed())
            .map(|(construct, _)| *construct)
            .collect();

        let reported = match ast.validate(profile) {
            Ok(()) => Vec::new(),
            Err(violations) => violations
                .iter()
                .filter_map(|v| {
                    let any = ast.resolve_node_id(v.node)?;
                    let id = any.as_type()?;
                    let node = ast.type_(id).ok()?;
                    Some(ConstructKind::of(&node.kind))
                })
                .collect::<Vec<_>>(),
        };

        for construct in &expected {
            assert!(
                reported.contains(construct),
                "{profile:?} should have rejected {construct:?}"
            );
        }
        for construct in &reported {
            assert!(
                expected.contains(construct)
                    || !construct_verdict(profile, *construct).is_allowed(),
                "{profile:?} rejected {construct:?}, which the table allows"
            );
        }
    }
}

#[test]
fn every_facet_property_is_reachable_from_a_real_node() {
    // A property nothing can carry would have dead rulings.
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "L", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("L");
    let s = ast
        .add_type(
            root,
            "Carrier",
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("Carrier");
    let member = ast
        .add_member(s, MemberSpec::new("x", Ref::Resolved(long)))
        .expect("x");
    let expr = ast.add_int_expr(1).expect("1");
    let any = ast.any_of_member(member).expect("any");

    {
        let facets = ast.facets_mut(any).expect("facets");
        let xtypes = facets.xtypes_mut();
        xtypes.is_key = true;
        xtypes.is_optional = true;
        xtypes.is_external = true;
        xtypes.is_must_understand = true;
        xtypes.is_non_serialized = true;
        xtypes.try_construct = Some(zerodds_ast::TryConstruct::UseDefault);
        xtypes.extensibility = Some(Extensibility::Mutable);
        xtypes.member_id = Some(zerodds_ast::MemberIdAssignment {
            effective: 1,
            origin: zerodds_ast::MemberIdOrigin::Declared,
        });
        xtypes.default_value = Some(expr);
        xtypes.range = Some(zerodds_ast::RangeConstraint {
            min: Some(expr),
            max: Some(expr),
        });
        xtypes.unit = Some("m/s".to_string());
        xtypes.verbatim.push(zerodds_ast::VerbatimDirective {
            language: "c++".to_string(),
            placement: zerodds_ast::VerbatimPlacement::BeginFile,
            text: "// hi".to_string(),
        });
        facets.ros2_mut().default_value = Some(expr);
        facets.vendor.rti = Some(zerodds_ast::RtiFacet::default());
    }

    // Every property is now set on one node, so a profile that forbids
    // any of them must complain about this member.
    let violations = ast
        .validate(Profile::Corba33)
        .expect_err("CORBA rejects most of these");
    assert!(
        violations.len() >= 10,
        "expected CORBA to reject most properties, got {}",
        violations.len()
    );

    // And the widest profile accepts every construct it rules on.
    let vendor_ok = ast.validate(Profile::Vendor(zerodds_ast::VendorKind::Rti));
    assert!(
        vendor_ok.is_ok(),
        "a vendor dialect admits what the model can hold: {vendor_ok:?}"
    );
}

/// A profile answers about the **source**, not about what lowering
/// derived from it.
///
/// `struct Sample { long a; double b; };` is CORBA IDL. Every member
/// of it carries a member id in the model, because XTypes requires one
/// and the default rule is the previous member's id plus one — a rule
/// no source ever writes. The `ExplicitMemberId` property asked
/// whether the model *had* an id rather than where the id came from,
/// so it answered yes for every aggregate ever lowered.
///
/// What that cost: `Profile::Corba33` admitted 2 of the 376 interop
/// corpus cases and `Profile::Ros2` admitted none, which made the only
/// instrument for asking "is this case even inside that toolchain's
/// specification" unusable. Nine toolchains were then measured against
/// the whole corpus instead — a corpus of XTypes constructs — and TAO
/// was recorded as refusing 127 cases that CORBA 3.3 does not define.
/// With the origin asked for, CORBA admits 199 and ROS 2 admits 146,
/// and the ORBs carry 177 of their 199.
#[test]
fn a_default_member_id_is_not_an_explicit_one() {
    for (origin, explicit) in [
        (MemberIdOrigin::PreviousPlusOne, false),
        (MemberIdOrigin::Declared, true),
        (MemberIdOrigin::AutoSequential, true),
        (MemberIdOrigin::AutoHash, true),
    ] {
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
        let member = ast
            .add_member(s, MemberSpec::new("m", Ref::Resolved(long)))
            .expect("m");
        let any = ast.any_of_member(member).expect("any");
        ast.facets_mut(any).expect("facets").xtypes_mut().member_id = Some(MemberIdAssignment {
            effective: 0,
            origin: origin.clone(),
        });
        // The mutation this catches: `member_id.is_some()`, which
        // answers `true` for all four origins and makes the first
        // line of the loop fail.
        assert_eq!(
            ast.validate(Profile::Corba33).is_err(),
            explicit,
            "{origin:?} should {} put the type outside CORBA 3.3",
            if explicit { "" } else { "not" }
        );
    }
}
