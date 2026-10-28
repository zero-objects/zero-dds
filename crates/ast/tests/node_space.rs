// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The node space of spec §3.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::node::annotation::BUILTIN_ANNOTATIONS;
use zerodds_ast::node::constant::BinaryOp;
use zerodds_ast::{
    Ast, Bound, CaseLabel, CharWidth, Completeness, ConstExpr, EndpointRole, EvalError,
    InterfaceKind, Literal, MemberSpec, NodeKind, ParamDirection, PrimitiveKind, QosPolicy,
    QosPolicyKind, TypeKind, UnaryOp, ValuetypeKind,
};
use zerodds_ast_core::{IdSpace, PendingRef, Ref, ScopedName};

fn scoped(segments: &[&str]) -> ScopedName {
    ScopedName::absolute(segments.iter().map(|s| (*s).to_string()).collect())
}

fn pending(segments: &[&str]) -> Ref<zerodds_ast::TypeNode> {
    Ref::Pending(PendingRef::ScopedName(scoped(segments)))
}

// -- modules, spec §3.1 --------------------------------------------------

#[test]
fn a_fresh_model_has_an_anonymous_root_module() {
    let ast = Ast::new();
    let module = ast.module(ast.root_module()).expect("root module");
    assert_eq!(module.name, "");
    assert_eq!(module.parent, None);
    assert!(module.is_root());
    assert!(module.children.is_empty());
}

#[test]
fn nested_modules_are_addressable_by_path() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let robot = ast.ensure_module(root, "Robot").expect("Robot");
    let sensors = ast.ensure_module(robot, "Sensors").expect("Sensors");

    assert_eq!(
        ast.module_by_path(&scoped(&["Robot", "Sensors"])),
        Some(sensors)
    );
    assert_eq!(ast.module_by_path(&scoped(&["Robot"])), Some(robot));
}

#[test]
fn reopening_a_module_merges_into_the_same_node() {
    let mut ast = Ast::new();
    let root = ast.root_module();

    // Two translation units both open `module Robot`.
    let first = ast.ensure_module(root, "Robot").expect("first open");
    let second = ast.ensure_module(root, "Robot").expect("second open");

    assert_eq!(first, second);
    assert_eq!(ast.module(root).expect("root").children.len(), 1);
}

#[test]
fn a_module_records_a_stable_identity_that_maps_back() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let robot = ast.ensure_module(root, "Robot").expect("Robot");

    let node_id = ast.module(robot).expect("Robot").node_id;
    let back = ast.resolve_node_id(node_id).expect("reverse lookup");
    assert_eq!(back.kind(), NodeKind::Module);
    assert_eq!(back.node_id(), node_id);
    assert_eq!(back.as_module(), Some(robot));
}

#[test]
fn an_unknown_path_resolves_to_nothing() {
    let ast = Ast::new();
    assert_eq!(ast.module_by_path(&scoped(&["Nope"])), None);
}

// -- types, spec §3.2 ----------------------------------------------------

#[test]
fn every_primitive_of_the_spec_table_is_representable() {
    // Spec §3.2, primitive row: fifteen entries, no more, no fewer.
    assert_eq!(PrimitiveKind::ALL.len(), 15);
    assert!(PrimitiveKind::ALL.contains(&PrimitiveKind::Float128));
    assert!(PrimitiveKind::ALL.contains(&PrimitiveKind::Int8));
    assert!(PrimitiveKind::ALL.contains(&PrimitiveKind::WChar));
    // No duplicates.
    let mut sorted = PrimitiveKind::ALL;
    sorted.sort_unstable();
    sorted.windows(2).for_each(|w| assert_ne!(w[0], w[1]));
}

#[test]
fn every_node_kind_tag_round_trips_and_no_two_collide() {
    let mut tags: Vec<u16> = NodeKind::ALL.iter().map(|k| k.tag().0).collect();
    let count = tags.len();
    tags.sort_unstable();
    tags.dedup();
    assert_eq!(tags.len(), count, "two node kinds share a tag");

    for kind in NodeKind::ALL {
        assert_eq!(NodeKind::from_tag(kind.tag()), Some(kind));
    }
}

#[test]
fn a_type_is_addressable_and_listed_among_its_module_children() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let robot = ast.ensure_module(root, "Robot").expect("Robot");
    let id = ast
        .add_type(robot, "Score", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("Score");

    let node = ast.type_(id).expect("Score");
    assert_eq!(node.name, "Score");
    assert_eq!(node.module, robot);
    assert_eq!(node.completeness, Completeness::Complete);
    assert_eq!(
        ast.resolve_node_id(node.node_id).expect("reverse").kind(),
        NodeKind::Type
    );
    assert!(
        ast.module(robot)
            .expect("Robot")
            .children
            .iter()
            .any(|c| c.node_id() == node.node_id)
    );
}

#[test]
fn a_sequence_can_reference_a_type_that_has_not_arrived_yet() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let id = ast
        .add_type(
            root,
            "PoseSeq",
            TypeKind::Sequence {
                element: pending(&["Robot", "Pose"]),
                bound: Bound::Unbounded,
            },
        )
        .expect("PoseSeq");

    let TypeKind::Sequence { element, bound } = &ast.type_(id).expect("PoseSeq").kind else {
        panic!("expected a sequence");
    };
    assert!(element.is_pending());
    assert_eq!(*bound, Bound::Unbounded);
    assert!(!bound.is_bounded());
}

#[test]
fn narrow_and_wide_strings_are_distinct_kinds() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let narrow = ast
        .add_type(
            root,
            "Name",
            TypeKind::String {
                width: CharWidth::Char,
                bound: Bound::Unbounded,
            },
        )
        .expect("narrow");
    let wide = ast
        .add_type(
            root,
            "WideName",
            TypeKind::String {
                width: CharWidth::WChar,
                bound: Bound::Unbounded,
            },
        )
        .expect("wide");

    assert_ne!(
        ast.type_(narrow).expect("n").kind,
        ast.type_(wide).expect("w").kind
    );
}

#[test]
fn fixed_retains_digits_and_scale() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let id = ast
        .add_type(
            root,
            "Money",
            TypeKind::Fixed {
                digits: 15,
                scale: 2,
            },
        )
        .expect("Money");
    assert!(matches!(
        ast.type_(id).expect("Money").kind,
        TypeKind::Fixed {
            digits: 15,
            scale: 2
        }
    ));
}

#[test]
fn a_forward_declared_type_completes_without_invalidating_handles() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let id = ast
        .add_forward_type(
            root,
            "Pose",
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("forward Pose");
    assert_eq!(
        ast.type_(id).expect("Pose").completeness,
        Completeness::Forward
    );

    // A member added later completes the body; the handle stays valid.
    let long = ast
        .add_type(root, "L", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("L");
    let _ = ast
        .add_member(id, MemberSpec::new("x", Ref::Resolved(long)))
        .expect("member x");
    assert_eq!(ast.type_(id).expect("Pose").kind.members().len(), 1);
}

#[test]
fn every_valuetype_modifier_is_representable() {
    assert_eq!(ValuetypeKind::ALL.len(), 4);
    assert!(ValuetypeKind::ALL.contains(&ValuetypeKind::Truncatable));
    assert!(ValuetypeKind::ALL.contains(&ValuetypeKind::Custom));
}

// -- members and aggregate parts, spec §3.3 ------------------------------

#[test]
fn struct_members_land_in_declaration_order_with_the_right_parent() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "L", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("L");
    let s = ast
        .add_type(
            root,
            "Point",
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("Point");

    let x = ast
        .add_member(s, MemberSpec::new("x", Ref::Resolved(long)))
        .expect("x");
    let y = ast
        .add_member(s, MemberSpec::new("y", Ref::Resolved(long)))
        .expect("y");

    assert_eq!(ast.type_(s).expect("Point").kind.members(), &[x, y]);
    let parent = ast.member(x).expect("x").parent;
    assert_eq!(parent.as_type(), Some(s));
}

#[test]
fn adding_a_member_to_a_non_aggregate_is_rejected() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "L", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("L");
    let e = ast
        .add_type(
            root,
            "Colour",
            TypeKind::Enumeration {
                enumerators: Vec::new(),
            },
        )
        .expect("Colour");

    let before = ast.node_count();
    assert!(
        ast.add_member(e, MemberSpec::new("x", Ref::Resolved(long)))
            .is_err()
    );
    // The rejected member left nothing behind.
    assert_eq!(ast.node_count(), before);
}

#[test]
fn a_positional_enumerator_and_an_explicit_value_are_distinguishable() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let e = ast
        .add_type(
            root,
            "Colour",
            TypeKind::Enumeration {
                enumerators: Vec::new(),
            },
        )
        .expect("Colour");

    let red = ast.add_enumerator(e, "RED", None).expect("RED");
    let expr = ast.add_int_expr(7).expect("7");
    let green = ast.add_enumerator(e, "GREEN", Some(expr)).expect("GREEN");

    assert!(ast.enumerator(red).expect("RED").value.is_none());
    assert_eq!(ast.enumerator(green).expect("GREEN").value, Some(expr));
}

#[test]
fn a_positional_bit_and_an_explicit_position_are_distinguishable() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let b = ast
        .add_type(root, "Flags", TypeKind::Bitmask { bits: Vec::new() })
        .expect("Flags");

    let a = ast.add_bit_value(b, "A", None).expect("A");
    let c = ast.add_bit_value(b, "C", Some(5)).expect("C");

    assert!(ast.bit_value(a).expect("A").position.is_none());
    assert_eq!(ast.bit_value(c).expect("C").position, Some(5));
}

#[test]
fn a_default_case_is_recorded_on_the_union_as_well_as_in_the_case_list() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "L", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("L");
    let u = ast
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

    let label = ast.add_int_expr(1).expect("1");
    let first = ast
        .add_case(
            u,
            vec![CaseLabel::Value(label)],
            MemberSpec::new("a", Ref::Resolved(long)),
        )
        .expect("case 1");
    let fallback = ast
        .add_case(
            u,
            vec![CaseLabel::Default],
            MemberSpec::new("other", Ref::Resolved(long)),
        )
        .expect("default case");

    let TypeKind::Union {
        cases,
        default_case,
        ..
    } = &ast.type_(u).expect("Value").kind
    else {
        panic!("expected a union");
    };
    assert_eq!(cases, &[first, fallback]);
    assert_eq!(*default_case, Some(fallback));
}

#[test]
fn a_case_may_carry_several_labels() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "L", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("L");
    let u = ast
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

    let one = ast.add_int_expr(1).expect("1");
    let two = ast.add_int_expr(2).expect("2");
    let c = ast
        .add_case(
            u,
            vec![CaseLabel::Value(one), CaseLabel::Value(two)],
            MemberSpec::new("a", Ref::Resolved(long)),
        )
        .expect("case 1,2");
    assert_eq!(ast.case(c).expect("case").labels.len(), 2);
}

#[test]
fn an_anonymous_bitfield_carries_no_name() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let b = ast
        .add_type(
            root,
            "Header",
            TypeKind::Bitset {
                base: None,
                fields: Vec::new(),
            },
        )
        .expect("Header");

    let named = ast.add_bitfield(b, Some("flag"), 1, None).expect("flag");
    let padding = ast.add_bitfield(b, None, 3, None).expect("padding");

    assert_eq!(
        ast.bitfield(named).expect("flag").name.as_deref(),
        Some("flag")
    );
    assert!(ast.bitfield(padding).expect("padding").is_padding());
}

#[test]
fn a_member_may_reference_a_type_that_has_not_arrived() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let s = ast
        .add_type(
            root,
            "Robot",
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("Robot");
    let m = ast
        .add_member(s, MemberSpec::new("pose", pending(&["Robot", "Pose"])))
        .expect("pose");
    assert!(ast.member(m).expect("pose").type_.is_pending());
}

// -- constants, spec §3.5 ------------------------------------------------

#[test]
fn an_expression_tree_survives_as_a_tree() {
    let mut ast = Ast::new();
    // 2 * (3 + 4)
    let two = ast.add_int_expr(2).expect("2");
    let three = ast.add_int_expr(3).expect("3");
    let four = ast.add_int_expr(4).expect("4");
    let sum = ast
        .add_expr(ConstExpr::Binary {
            op: BinaryOp::Add,
            lhs: three,
            rhs: four,
        })
        .expect("3 + 4");
    let product = ast
        .add_expr(ConstExpr::Binary {
            op: BinaryOp::Mul,
            lhs: two,
            rhs: sum,
        })
        .expect("2 * (3 + 4)");

    // The root is still a multiplication, not a folded literal 14.
    let ConstExpr::Binary { op, rhs, .. } = &ast.expr(product).expect("root").expr else {
        panic!("expected a binary expression");
    };
    assert_eq!(*op, BinaryOp::Mul);
    assert!(matches!(
        ast.expr(*rhs).expect("rhs").expr,
        ConstExpr::Binary { .. }
    ));
}

#[test]
fn evaluation_yields_the_folded_value_without_mutating_the_tree() {
    let mut ast = Ast::new();
    let two = ast.add_int_expr(2).expect("2");
    let three = ast.add_int_expr(3).expect("3");
    let four = ast.add_int_expr(4).expect("4");
    let sum = ast
        .add_expr(ConstExpr::Binary {
            op: BinaryOp::Add,
            lhs: three,
            rhs: four,
        })
        .expect("sum");
    let product = ast
        .add_expr(ConstExpr::Binary {
            op: BinaryOp::Mul,
            lhs: two,
            rhs: sum,
        })
        .expect("product");

    assert_eq!(ast.eval_integral(product), Ok(14));
    assert!(matches!(
        ast.expr(product).expect("root").expr,
        ConstExpr::Binary {
            op: BinaryOp::Mul,
            ..
        }
    ));
}

#[test]
fn division_by_zero_is_an_error_not_a_panic() {
    let mut ast = Ast::new();
    let one = ast.add_int_expr(1).expect("1");
    let zero = ast.add_int_expr(0).expect("0");
    let div = ast
        .add_expr(ConstExpr::Binary {
            op: BinaryOp::Div,
            lhs: one,
            rhs: zero,
        })
        .expect("1 / 0");
    assert!(matches!(
        ast.eval_integral(div),
        Err(EvalError::DivisionByZero { .. })
    ));

    let rem = ast
        .add_expr(ConstExpr::Binary {
            op: BinaryOp::Mod,
            lhs: one,
            rhs: zero,
        })
        .expect("1 % 0");
    assert!(matches!(
        ast.eval_integral(rem),
        Err(EvalError::DivisionByZero { .. })
    ));
}

#[test]
fn overflow_is_an_error_not_a_wrap() {
    let mut ast = Ast::new();
    let max = ast.add_int_expr(i128::MAX).expect("max");
    let one = ast.add_int_expr(1).expect("1");
    let sum = ast
        .add_expr(ConstExpr::Binary {
            op: BinaryOp::Add,
            lhs: max,
            rhs: one,
        })
        .expect("max + 1");
    assert!(matches!(
        ast.eval_integral(sum),
        Err(EvalError::Overflow { .. })
    ));
}

#[test]
fn evaluating_through_an_unresolved_name_fails_loudly() {
    let mut ast = Ast::new();
    let reference = ast
        .add_expr(ConstExpr::NameRef(Ref::Pending(PendingRef::ScopedName(
            scoped(&["MAX_LEN"]),
        ))))
        .expect("name ref");
    // Not zero, not a default: an error naming the unresolved node.
    assert!(matches!(
        ast.eval_integral(reference),
        Err(EvalError::Unresolved { .. })
    ));
}

#[test]
fn an_over_deep_expression_is_rejected_rather_than_run_unbounded() {
    let mut ast = Ast::new();
    let mut current = ast.add_int_expr(1).expect("leaf");
    for _ in 0..300 {
        current = ast
            .add_expr(ConstExpr::Unary {
                op: UnaryOp::Minus,
                operand: current,
            })
            .expect("negation");
    }
    assert_eq!(ast.eval_integral(current), Err(EvalError::TooDeep));
}

#[test]
fn float_literals_retain_their_written_form() {
    let bare = Literal::Float {
        mantissa: "1.".to_string(),
        exponent: None,
    };
    let zeroed = Literal::Float {
        mantissa: "1.0".to_string(),
        exponent: None,
    };
    assert_ne!(bare, zeroed);
}

#[test]
fn every_operator_of_idl_4_2_is_representable() {
    assert_eq!(BinaryOp::ALL.len(), 10);
    assert_eq!(UnaryOp::ALL.len(), 3);
    assert!(BinaryOp::ALL.contains(&BinaryOp::Shl));
    assert!(BinaryOp::ALL.contains(&BinaryOp::Mod));
}

#[test]
fn a_constant_reference_evaluates_through_to_its_definition() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "L", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("L");
    let value = ast.add_int_expr(42).expect("42");
    let max_len = ast
        .add_const(root, "MAX_LEN", Ref::Resolved(long), value)
        .expect("MAX_LEN");

    let reference = ast
        .add_expr(ConstExpr::NameRef(Ref::Resolved(max_len)))
        .expect("reference");
    assert_eq!(ast.eval_integral(reference), Ok(42));
}

// -- interfaces, spec §3.4 -----------------------------------------------

#[test]
fn all_seven_interface_kinds_are_representable() {
    assert_eq!(InterfaceKind::ALL.len(), 7);
    assert!(InterfaceKind::Component.is_component_model());
    assert!(!InterfaceKind::Interface.is_component_model());
}

#[test]
fn an_operation_records_its_parameters_in_order_with_directions() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "L", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("L");
    let iface = ast
        .add_interface(root, "Control", InterfaceKind::Interface, Vec::new())
        .expect("Control");
    let op = ast
        .add_operation(iface, "move", Some(Ref::Resolved(long)), false)
        .expect("move");

    let _ = ast
        .add_param(op, "a", ParamDirection::In, Ref::Resolved(long))
        .expect("a");
    let _ = ast
        .add_param(op, "b", ParamDirection::Out, Ref::Resolved(long))
        .expect("b");
    let _ = ast
        .add_param(op, "c", ParamDirection::InOut, Ref::Resolved(long))
        .expect("c");

    let directions: Vec<ParamDirection> = ast
        .operation(op)
        .expect("move")
        .params
        .iter()
        .map(|p| ast.param(*p).expect("param").direction)
        .collect();
    assert_eq!(
        directions,
        vec![
            ParamDirection::In,
            ParamDirection::Out,
            ParamDirection::InOut
        ]
    );
}

#[test]
fn a_oneway_operation_is_flagged_and_a_normal_one_is_not() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "L", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("L");
    let iface = ast
        .add_interface(root, "Control", InterfaceKind::Interface, Vec::new())
        .expect("Control");

    let fire = ast
        .add_operation(iface, "notify", Some(Ref::Resolved(long)), true)
        .expect("notify");
    let call = ast
        .add_operation(iface, "query", Some(Ref::Resolved(long)), false)
        .expect("query");

    assert!(ast.operation(fire).expect("notify").oneway);
    assert!(!ast.operation(call).expect("query").oneway);
}

#[test]
fn interface_inheritance_records_every_base_in_order() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let a = ast
        .add_interface(root, "A", InterfaceKind::Interface, Vec::new())
        .expect("A");
    let b = ast
        .add_interface(root, "B", InterfaceKind::Interface, Vec::new())
        .expect("B");
    let c = ast
        .add_interface(
            root,
            "C",
            InterfaceKind::Interface,
            vec![Ref::Resolved(a), Ref::Resolved(b)],
        )
        .expect("C");

    // Order matters: CORBA multiple inheritance resolves lookup along it.
    assert_eq!(
        ast.interface(c).expect("C").bases,
        vec![Ref::Resolved(a), Ref::Resolved(b)]
    );
}

/// `operations_of` answers what an interface declares; `all_operations_of`
/// answers what it has. OMG IDL 4.2 §7.4.3 gives a derived interface
/// every operation of its bases, and every language adapter that walked
/// the declared list alone generated a binding a client could not call
/// the inherited ones through — with nothing to say the operation had
/// been left out.
///
/// The bases come first and in declaration order, which is the order
/// §7.4.3 resolves a lookup along, and a base reached twice through a
/// diamond contributes its operations once.
#[test]
fn all_operations_of_answers_with_the_bases_operations_first() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let top = ast
        .add_interface(root, "Top", InterfaceKind::Interface, Vec::new())
        .expect("Top");
    let t = ast.add_operation(top, "t", None, false).expect("t");
    let left = ast
        .add_interface(
            root,
            "Left",
            InterfaceKind::Interface,
            vec![Ref::Resolved(top)],
        )
        .expect("Left");
    let l = ast.add_operation(left, "l", None, false).expect("l");
    let right = ast
        .add_interface(
            root,
            "Right",
            InterfaceKind::Interface,
            vec![Ref::Resolved(top)],
        )
        .expect("Right");
    let r = ast.add_operation(right, "r", None, false).expect("r");
    let bottom = ast
        .add_interface(
            root,
            "Bottom",
            InterfaceKind::Interface,
            vec![Ref::Resolved(left), Ref::Resolved(right)],
        )
        .expect("Bottom");
    let b = ast.add_operation(bottom, "b", None, false).expect("b");

    assert_eq!(ast.operations_of(bottom), &[b]);
    assert_eq!(ast.all_operations_of(bottom), vec![t, l, r, b]);
    assert_eq!(ast.all_operations_of(top), vec![t]);
}

#[test]
fn a_readonly_attribute_records_the_flag_without_rejecting_anything_here() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "L", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("L");
    let iface = ast
        .add_interface(root, "Control", InterfaceKind::Interface, Vec::new())
        .expect("Control");
    let attr = ast
        .add_attribute(iface, "speed", Ref::Resolved(long), true)
        .expect("speed");

    let node = ast.attribute(attr).expect("speed");
    assert!(node.readonly);
    // Rejecting a set_raises on a readonly attribute is profile
    // validation's job, not the model's.
    assert!(node.set_raises.is_empty());
}

#[test]
fn an_exception_carries_members_like_any_aggregate() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "L", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("L");
    let ex = ast.add_exception(root, "Failure").expect("Failure");
    let m = ast
        .add_exception_member(ex, MemberSpec::new("code", Ref::Resolved(long)))
        .expect("code");

    assert_eq!(ast.exception(ex).expect("Failure").members, vec![m]);
    assert_eq!(
        ast.member(m).expect("code").parent.kind(),
        NodeKind::Exception
    );
}

// -- annotations, spec §3.6 ----------------------------------------------

#[test]
fn every_builtin_annotation_of_the_spec_list_is_present() {
    let ast = Ast::new();
    // 40 since DDS-RPC 1.0's nine annotations (`@DDSRequestTopic`,
    // `@DDSReplyTopic`, `@RPCRequestType`, `@RPCReplyType`, `@Qos`,
    // `@Choice`, `@Autoid`, `@Empty`, `@Mutable`) joined the list. The
    // number is written out so that adding an entry is a deliberate act:
    // the order allocates builtin identities and a snapshot from another
    // build has to compare, which is why a new name is appended and never
    // inserted.
    assert_eq!(BUILTIN_ANNOTATIONS.len(), 40);
    for name in BUILTIN_ANNOTATIONS {
        assert!(
            ast.builtin_annotation(name).is_some(),
            "built-in annotation @{name} is missing"
        );
    }
}

#[test]
fn builtin_identities_come_from_the_builtin_space() {
    let ast = Ast::new();
    let key = ast.builtin_annotation("key").expect("@key");
    let node = ast.annotation_def(key).expect("@key");
    assert_eq!(node.node_id.space(), IdSpace::BUILTIN);
    assert_eq!(node.module, ast.builtin_module());
}

#[test]
fn two_fresh_models_agree_on_every_builtin_identity() {
    let first = Ast::new();
    let second = Ast::new();
    for name in BUILTIN_ANNOTATIONS {
        let a = first.builtin_annotation(name).expect("first");
        let b = second.builtin_annotation(name).expect("second");
        assert_eq!(
            first.annotation_def(a).expect("a").node_id,
            second.annotation_def(b).expect("b").node_id,
            "built-in @{name} differs between two fresh models"
        );
    }
}

#[test]
fn builtin_identities_do_not_collide_with_parsed_content() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let user = ast
        .add_type(root, "Score", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("Score");
    assert_eq!(
        ast.type_(user).expect("Score").node_id.space(),
        IdSpace::MODEL
    );
}

#[test]
fn a_user_annotation_traverses_exactly_like_a_builtin() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let mine = ast.add_annotation_def(root, "my_ann").expect("@my_ann");
    let node = ast.annotation_def(mine).expect("@my_ann");
    assert_eq!(
        ast.resolve_node_id(node.node_id).expect("reverse").kind(),
        NodeKind::AnnotationDef
    );
}

#[test]
fn the_value_member_shorthand_is_recorded() {
    let ast = Ast::new();
    let id = ast.builtin_annotation("id").expect("@id");
    let def = ast.annotation_def(id).expect("@id");
    let member = ast
        .annotation_member(*def.members.first().expect("value member"))
        .expect("value member");
    assert_eq!(member.name, "value");
    assert!(member.is_value_member);
}

// -- deployment, spec §3.7 -----------------------------------------------

#[test]
fn all_twenty_four_qos_policies_are_representable() {
    assert_eq!(QosPolicyKind::ALL.len(), 24);
    let mut sorted = QosPolicyKind::ALL;
    sorted.sort_unstable();
    sorted.windows(2).for_each(|w| assert_ne!(w[0], w[1]));
    assert!(QosPolicyKind::ALL.contains(&QosPolicyKind::DataRepresentation));
    assert!(QosPolicyKind::ALL.contains(&QosPolicyKind::TypeConsistencyEnforcement));
}

#[test]
fn a_topic_binds_a_type_and_a_qos_profile() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let pose = ast
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
        .add_qos_profile(lib, "Reliable", None, Vec::new())
        .expect("profile");
    let domain = ast.add_domain(lib, "Main", 0).expect("domain");
    let topic = ast
        .add_topic(
            domain,
            "RobotPose",
            Ref::Resolved(pose),
            Some(Ref::Resolved(profile)),
        )
        .expect("topic");

    let node = ast.topic(topic).expect("topic");
    assert_eq!(node.type_, Ref::Resolved(pose));
    assert_eq!(node.qos, Some(Ref::Resolved(profile)));
    assert_eq!(ast.domain(domain).expect("domain").topics, vec![topic]);
}

#[test]
fn a_profile_may_inherit_from_another_profile() {
    let mut ast = Ast::new();
    let lib = ast.add_library("Deployment").expect("library");
    let base = ast
        .add_qos_profile(lib, "Base", None, Vec::new())
        .expect("base");
    let derived = ast
        .add_qos_profile(lib, "Derived", Some(Ref::Resolved(base)), Vec::new())
        .expect("derived");

    assert_eq!(
        ast.qos_profile(derived).expect("derived").base,
        Some(Ref::Resolved(base))
    );
}

#[test]
fn an_unwritten_policy_field_stays_distinguishable_from_a_default_one() {
    let mut ast = Ast::new();
    let lib = ast.add_library("Deployment").expect("library");
    let profile = ast
        .add_qos_profile(
            lib,
            "Partial",
            None,
            vec![QosPolicy::Reliability {
                kind: None,
                max_blocking_time: None,
            }],
        )
        .expect("profile");

    let QosPolicy::Reliability { kind, .. } =
        &ast.qos_profile(profile).expect("profile").policies[0]
    else {
        panic!("expected reliability");
    };
    // Not substituted with RELIABLE: the field was never written, and a
    // profile inheriting from this one needs to know that.
    assert!(kind.is_none());
}

#[test]
fn a_reader_and_a_writer_endpoint_are_distinct_roles() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let pose = ast
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
    let domain = ast.add_domain(lib, "Main", 0).expect("domain");
    let topic = ast
        .add_topic(domain, "RobotPose", Ref::Resolved(pose), None)
        .expect("topic");
    let participant = ast
        .add_participant(lib, "P", Ref::Resolved(domain), None)
        .expect("participant");

    let writer = ast
        .add_endpoint(
            participant,
            "w",
            EndpointRole::DataWriter,
            Ref::Resolved(topic),
            None,
        )
        .expect("writer");
    let reader = ast
        .add_endpoint(
            participant,
            "r",
            EndpointRole::DataReader,
            Ref::Resolved(topic),
            None,
        )
        .expect("reader");

    assert_ne!(
        ast.endpoint(writer).expect("w").role,
        ast.endpoint(reader).expect("r").role
    );
    assert_eq!(
        ast.participant(participant).expect("P").endpoints,
        vec![writer, reader]
    );
}

#[test]
fn a_topic_may_reference_a_type_that_has_not_been_discovered_yet() {
    let mut ast = Ast::new();
    let lib = ast.add_library("Deployment").expect("library");
    let domain = ast.add_domain(lib, "Main", 0).expect("domain");
    let topic = ast
        .add_topic(domain, "RobotPose", pending(&["Robot", "Pose"]), None)
        .expect("topic");
    assert!(ast.topic(topic).expect("topic").type_.is_pending());
}

// -- identity namespaces, spec §1.2 --------------------------------------

#[test]
fn a_fresh_model_reports_no_commits_yet() {
    let ast = Ast::new();
    assert_eq!(ast.commit_sequence(), 0);
}

#[test]
fn synthetic_identities_live_in_their_own_namespace() {
    let mut ast = Ast::new();
    let a = ast.allocate_synthetic_id().expect("synthetic a");
    let b = ast.allocate_synthetic_id().expect("synthetic b");
    assert_eq!(a.space(), IdSpace::SYNTHETIC);
    assert!(b > a);

    // A parsed node's identity never collides with a synthetic one.
    let root = ast.root_module();
    let parsed = ast
        .add_type(root, "Score", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("Score");
    assert_ne!(ast.type_(parsed).expect("Score").node_id, a);
}
