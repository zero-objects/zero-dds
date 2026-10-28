// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Facets, the transactional edit surface and structural invariants.
//! Spec §4, §5.1, §8.1.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use zerodds_ast::facet::vendor::RtiFacet;
use zerodds_ast::{
    Ast, AstError, Extensibility, Facets, InvariantViolation, MemberIdAssignment, MemberIdOrigin,
    MemberSpec, PrimitiveKind, RosInterfaceRole, SourceOrigin, TryConstruct, TypeKind,
    VerbatimPlacement,
};
use zerodds_ast_core::{AstObserver, Changeset, Ref};

// -- helpers -------------------------------------------------------------

struct Recorder {
    calls: AtomicUsize,
    changes: AtomicUsize,
}

impl Recorder {
    fn new() -> Self {
        Self {
            calls: AtomicUsize::new(0),
            changes: AtomicUsize::new(0),
        }
    }
}

impl AstObserver for Recorder {
    fn on_commit(&self, changeset: &Changeset) {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.changes.fetch_add(changeset.len(), Ordering::SeqCst);
    }
}

/// A model with a `long` type and an empty struct, the shape most of
/// these tests start from.
fn model_with_struct() -> (
    Ast,
    zerodds_ast_core::Id<zerodds_ast::TypeNode>,
    zerodds_ast_core::Id<zerodds_ast::TypeNode>,
) {
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
    (ast, long, s)
}

// -- facets, spec §4 -----------------------------------------------------

#[test]
fn a_node_created_programmatically_records_a_synthetic_origin_not_none() {
    let (ast, long, _) = model_with_struct();
    let facets = &ast.type_(long).expect("L").facets;
    assert!(matches!(
        facets.source.origin,
        SourceOrigin::Synthetic { .. }
    ));
}

#[test]
fn the_effective_member_id_and_its_origin_are_both_retained() {
    let (mut ast, long, s) = model_with_struct();
    let m = ast
        .add_member(s, MemberSpec::new("x", Ref::Resolved(long)))
        .expect("x");
    let any = ast.any_of_member(m).expect("any");
    ast.facets_mut(any).expect("facets").xtypes_mut().member_id = Some(MemberIdAssignment {
        effective: 7,
        origin: MemberIdOrigin::Hashed {
            source_name: "x".to_string(),
        },
    });

    let facets = ast.facets(any).expect("facets");
    // Layout needs the number, a collision diagnostic needs the origin.
    assert_eq!(facets.effective_member_id(), Some(7));
    let assignment = facets
        .xtypes
        .as_ref()
        .and_then(|x| x.member_id.as_ref())
        .expect("assignment");
    assert!(matches!(assignment.origin, MemberIdOrigin::Hashed { .. }));
    assert_eq!(assignment.origin.as_str(), "@hashid");
}

#[test]
fn the_annotation_enumerations_are_complete() {
    assert_eq!(Extensibility::ALL.len(), 3);
    assert_eq!(TryConstruct::ALL.len(), 3);
    assert_eq!(VerbatimPlacement::ALL.len(), 6);
    assert_eq!(RosInterfaceRole::ALL.len(), 6);
}

#[test]
fn facets_from_different_sources_coexist_on_one_node() {
    let (mut ast, long, s) = model_with_struct();
    let m = ast
        .add_member(s, MemberSpec::new("x", Ref::Resolved(long)))
        .expect("x");
    let any = ast.any_of_member(m).expect("any");

    {
        let facets = ast.facets_mut(any).expect("facets");
        facets.xtypes_mut().is_key = true;
        facets.ros2_mut().was_constant = true;
        facets.corba_mut().native_mapping = Some("uint32_t".to_string());
    }

    // A ROS 2 message lowered to IDL and bridged to CORBA carries all
    // three at once, and none of them displaces another.
    let facets = ast.facets(any).expect("facets");
    assert!(facets.is_key());
    assert!(facets.ros2.as_ref().expect("ros2").was_constant);
    assert_eq!(
        facets
            .corba
            .as_ref()
            .expect("corba")
            .native_mapping
            .as_deref(),
        Some("uint32_t")
    );
}

#[test]
fn vendor_directives_are_not_normalised_into_core_constructs() {
    let (mut ast, _, s) = model_with_struct();
    let any = ast.any_of_type(s).expect("any");
    ast.facets_mut(any).expect("facets").vendor.rti = Some(RtiFacet {
        top_level: Some(true),
        ..RtiFacet::default()
    });

    let facets = ast.facets(any).expect("facets");
    assert_eq!(facets.vendor.present(), vec![zerodds_ast::VendorKind::Rti]);
    // The directive stayed where it was written; nothing leaked into the
    // XTypes facet, which is what a round-trip back to RTI depends on.
    assert!(facets.xtypes.is_none());
}

#[test]
fn an_absent_facet_is_none_not_a_default_populated_struct() {
    let facets = Facets::default();
    assert!(facets.xtypes.is_none());
    // A node with no XTypes information must not be read as claiming
    // Final extensibility.
    assert_eq!(facets.extensibility(), None);
    assert!(!facets.is_key());
    assert!(facets.vendor.is_empty());
}

// -- transactions, spec §5.1 ---------------------------------------------

#[test]
fn a_committed_transaction_emits_one_changeset_with_every_change() {
    let (mut ast, long, s) = model_with_struct();
    let recorder = Arc::new(Recorder::new());
    let _ = ast.observers_mut().register(recorder.clone());

    let mut tx = ast.begin();
    let _ = tx
        .ast()
        .add_member(s, MemberSpec::new("x", Ref::Resolved(long)))
        .expect("x");
    let _ = tx
        .ast()
        .add_member(s, MemberSpec::new("y", Ref::Resolved(long)))
        .expect("y");
    let changeset = tx.commit().expect("commit");

    // Two members created, and the struct that holds them updated for
    // each: a consumer tracking the struct has to hear about its part
    // list changing, not only about the parts.
    let created = changeset.iter().filter(|c| c.is_created()).count();
    let updated = changeset
        .iter()
        .filter(|c| matches!(c, zerodds_ast_core::Change::Updated { .. }))
        .count();
    assert_eq!(created, 2);
    assert_eq!(updated, 2);
    assert_eq!(recorder.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn a_direct_call_outside_a_transaction_commits_immediately() {
    let (mut ast, long, s) = model_with_struct();
    let recorder = Arc::new(Recorder::new());
    let _ = ast.observers_mut().register(recorder.clone());

    let _ = ast
        .add_member(s, MemberSpec::new("x", Ref::Resolved(long)))
        .expect("x");

    // The member's creation and the struct's part-list update are two
    // single-operation commits, because neither call opened a
    // transaction of its own.
    assert_eq!(recorder.calls.load(Ordering::SeqCst), 2);
    assert_eq!(recorder.changes.load(Ordering::SeqCst), 2);
}

#[test]
fn an_explicit_rollback_discards_every_change_and_notifies_nobody() {
    let (mut ast, long, s) = model_with_struct();
    let recorder = Arc::new(Recorder::new());
    let _ = ast.observers_mut().register(recorder.clone());
    let before = ast.node_count();

    let mut tx = ast.begin();
    let _ = tx
        .ast()
        .add_member(s, MemberSpec::new("x", Ref::Resolved(long)))
        .expect("x");
    tx.rollback();

    assert_eq!(ast.node_count(), before);
    assert!(ast.type_(s).expect("Point").kind.members().is_empty());
    assert_eq!(recorder.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn a_transaction_dropped_unfinished_rolls_back() {
    let (mut ast, long, s) = model_with_struct();
    let before = ast.node_count();

    {
        let mut tx = ast.begin();
        let _ = tx
            .ast()
            .add_member(s, MemberSpec::new("x", Ref::Resolved(long)))
            .expect("x");
        // No commit, no rollback: the transaction goes out of scope.
    }

    assert_eq!(ast.node_count(), before);
}

#[test]
fn a_rejected_commit_restores_the_model_and_notifies_nobody() {
    let (mut ast, long, s) = model_with_struct();
    let recorder = Arc::new(Recorder::new());
    let _ = ast.observers_mut().register(recorder.clone());
    let before = ast.node_count();

    let mut tx = ast.begin();
    let _ = tx
        .ast()
        .add_member(s, MemberSpec::new("x", Ref::Resolved(long)))
        .expect("first x");
    // The same name twice breaks a structural invariant.
    let _ = tx
        .ast()
        .add_member(s, MemberSpec::new("x", Ref::Resolved(long)))
        .expect("second x");
    let outcome = tx.commit();

    let Err(AstError::Invariant(violations)) = outcome else {
        panic!("expected the commit to be rejected");
    };
    assert!(
        violations
            .iter()
            .any(|v| matches!(v, InvariantViolation::DuplicateMemberName { .. }))
    );

    // Rolled back to exactly the prior state, and no observer saw it.
    assert_eq!(ast.node_count(), before);
    assert!(ast.type_(s).expect("Point").kind.members().is_empty());
    assert_eq!(recorder.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn observers_never_see_an_intermediate_state() {
    let (mut ast, long, s) = model_with_struct();

    struct Checker {
        calls: AtomicUsize,
    }
    impl AstObserver for Checker {
        fn on_commit(&self, changeset: &Changeset) {
            // Every notification carries a complete transaction, never a
            // partial one.
            assert!(!changeset.is_empty());
            self.calls.fetch_add(1, Ordering::SeqCst);
        }
    }

    let checker = Arc::new(Checker {
        calls: AtomicUsize::new(0),
    });
    let _ = ast.observers_mut().register(checker.clone());

    let mut tx = ast.begin();
    for name in ["a", "b", "c"] {
        let _ = tx
            .ast()
            .add_member(s, MemberSpec::new(name, Ref::Resolved(long)))
            .expect("member");
    }
    let _ = tx.commit().expect("commit");

    assert_eq!(checker.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn commit_sequence_numbers_increase_monotonically() {
    let (mut ast, long, s) = model_with_struct();
    let first = ast.commit_sequence();
    let _ = ast
        .add_member(s, MemberSpec::new("x", Ref::Resolved(long)))
        .expect("x");
    let second = ast.commit_sequence();
    let _ = ast
        .add_member(s, MemberSpec::new("y", Ref::Resolved(long)))
        .expect("y");
    let third = ast.commit_sequence();

    assert!(second > first);
    assert!(third > second);
}

// -- deletion, spec §5.2 -------------------------------------------------

#[test]
fn deleting_an_aggregate_cascades_to_its_members() {
    let (mut ast, long, s) = model_with_struct();
    let x = ast
        .add_member(s, MemberSpec::new("x", Ref::Resolved(long)))
        .expect("x");
    let y = ast
        .add_member(s, MemberSpec::new("y", Ref::Resolved(long)))
        .expect("y");

    let any = ast.any_of_type(s).expect("any");
    ast.remove(any).expect("remove Point");

    assert!(ast.type_(s).is_err());
    assert!(ast.member(x).is_err());
    assert!(ast.member(y).is_err());
}

#[test]
fn deleting_a_referenced_type_breaks_incoming_references_rather_than_dangling() {
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
    let seq = ast
        .add_type(
            root,
            "PoseSeq",
            TypeKind::Sequence {
                element: Ref::Resolved(pose),
                bound: zerodds_ast::Bound::Unbounded,
            },
        )
        .expect("PoseSeq");

    let any = ast.any_of_type(pose).expect("any");
    ast.remove(any).expect("remove Pose");

    let TypeKind::Sequence { element, .. } = &ast.type_(seq).expect("PoseSeq").kind else {
        panic!("expected a sequence");
    };
    // Broken, not a stale Resolved handle pointing at a freed slot.
    assert!(element.is_broken());
    assert!(!element.is_resolved());
}

#[test]
fn deleting_emits_one_deleted_change_per_removed_node() {
    let (mut ast, long, s) = model_with_struct();
    let _ = ast
        .add_member(s, MemberSpec::new("x", Ref::Resolved(long)))
        .expect("x");

    let counter = Arc::new(Recorder::new());
    let _ = ast.observers_mut().register(counter.clone());

    let any = ast.any_of_type(s).expect("any");
    ast.remove(any).expect("remove");

    // The struct and its one member.
    let deleted = counter.changes.load(Ordering::SeqCst);
    assert!(
        deleted >= 2,
        "expected at least the struct and its member, got {deleted}"
    );
}

#[test]
fn removing_an_unknown_identity_is_an_error() {
    let (mut ast, _, s) = model_with_struct();
    let any = ast.any_of_type(s).expect("any");
    ast.remove(any).expect("first remove");
    assert!(matches!(ast.remove(any), Err(AstError::NotFound { .. })));
}

// -- invariants, spec §8.1 -----------------------------------------------

#[test]
fn duplicate_member_ids_are_rejected_with_both_offenders_named() {
    let (mut ast, long, s) = model_with_struct();

    let mut tx = ast.begin();
    let x = tx
        .ast()
        .add_member(s, MemberSpec::new("x", Ref::Resolved(long)))
        .expect("x");
    let y = tx
        .ast()
        .add_member(s, MemberSpec::new("y", Ref::Resolved(long)))
        .expect("y");
    for member in [x, y] {
        let any = tx.ast().any_of_member(member).expect("any");
        tx.ast()
            .facets_mut(any)
            .expect("facets")
            .xtypes_mut()
            .member_id = Some(MemberIdAssignment {
            effective: 7,
            origin: MemberIdOrigin::Declared,
        });
    }
    let outcome = tx.commit();

    let Err(AstError::Invariant(violations)) = outcome else {
        panic!("expected rejection");
    };
    let collision = violations
        .iter()
        .find_map(|v| match v {
            InvariantViolation::DuplicateMemberId {
                first, second, id, ..
            } => Some((*first, *second, *id)),
            _ => None,
        })
        .expect("a duplicate-id violation");
    assert_eq!(collision.2, 7);
    // Both declarations are named, so a diagnostic can point at each.
    assert_ne!(collision.0, collision.1);
}

#[test]
fn a_zero_array_dimension_is_rejected() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "L", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("L");
    let zero = ast.add_int_expr(0).expect("0");

    let mut tx = ast.begin();
    let _ = tx
        .ast()
        .add_type(
            root,
            "Empty",
            TypeKind::Array {
                element: Ref::Resolved(long),
                dims: vec![zero],
            },
        )
        .expect("Empty");
    assert!(matches!(tx.commit(), Err(AstError::Invariant(_))));
}

#[test]
fn a_pending_reference_is_not_an_invariant_violation() {
    let mut ast = Ast::new();
    let root = ast.root_module();

    // Partial knowledge is a legal state, spec §2: this commits cleanly.
    let mut tx = ast.begin();
    let _ = tx
        .ast()
        .add_type(
            root,
            "PoseSeq",
            TypeKind::Sequence {
                element: Ref::Pending(zerodds_ast_core::PendingRef::ScopedName(
                    zerodds_ast_core::ScopedName::absolute(vec!["Pose".to_string()]),
                )),
                bound: zerodds_ast::Bound::Unbounded,
            },
        )
        .expect("PoseSeq");
    assert!(tx.commit().is_ok());
}

#[test]
fn a_broken_reference_is_not_an_invariant_violation_either() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let mut tx = ast.begin();
    let _ = tx
        .ast()
        .add_type(
            root,
            "PoseSeq",
            TypeKind::Sequence {
                element: Ref::Broken(zerodds_ast_core::BreakCause::NotFound {
                    what: zerodds_ast_core::PendingRef::ScopedName(
                        zerodds_ast_core::ScopedName::absolute(vec!["Pose".to_string()]),
                    ),
                }),
                bound: zerodds_ast::Bound::Unbounded,
            },
        )
        .expect("PoseSeq");
    // Reporting incompleteness is profile validation's job, not the
    // commit gate's.
    assert!(tx.commit().is_ok());
}

#[test]
fn a_duplicate_enumerator_name_is_rejected() {
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

    let mut tx = ast.begin();
    let _ = tx.ast().add_enumerator(e, "RED", None).expect("RED");
    let _ = tx.ast().add_enumerator(e, "RED", None).expect("RED again");
    assert!(matches!(tx.commit(), Err(AstError::Invariant(_))));
}

#[test]
fn two_default_cases_in_one_union_are_rejected() {
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

    let mut tx = ast.begin();
    let _ = tx
        .ast()
        .add_case(
            u,
            vec![zerodds_ast::CaseLabel::Default],
            MemberSpec::new("a", Ref::Resolved(long)),
        )
        .expect("first default");
    let _ = tx
        .ast()
        .add_case(
            u,
            vec![zerodds_ast::CaseLabel::Default],
            MemberSpec::new("b", Ref::Resolved(long)),
        )
        .expect("second default");
    assert!(matches!(tx.commit(), Err(AstError::Invariant(_))));
}

#[test]
fn a_label_on_a_string_discriminator_is_rejected_once_it_resolves() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "L", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("L");
    let text = ast
        .add_type(
            root,
            "S",
            TypeKind::String {
                width: zerodds_ast::CharWidth::Char,
                bound: zerodds_ast::Bound::Unbounded,
            },
        )
        .expect("S");
    let u = ast
        .add_type(
            root,
            "Value",
            TypeKind::Union {
                discriminator: Ref::Resolved(text),
                cases: Vec::new(),
                default_case: None,
            },
        )
        .expect("Value");
    let label = ast.add_int_expr(1).expect("1");

    let mut tx = ast.begin();
    let _ = tx
        .ast()
        .add_case(
            u,
            vec![zerodds_ast::CaseLabel::Value(label)],
            MemberSpec::new("a", Ref::Resolved(long)),
        )
        .expect("case");
    let outcome = tx.commit();
    let Err(AstError::Invariant(violations)) = outcome else {
        panic!("expected rejection");
    };
    assert!(
        violations
            .iter()
            .any(|v| matches!(v, InvariantViolation::IncompatibleCaseLabel { .. }))
    );
}

/// A **bitmask** is a discriminator type, and its cases carry ordinary
/// integral labels.
///
/// OMG IDL 4.2 §7.4.1.4.4.3 rule (50) writes `<switch_type_spec>` as
/// `<integer_type> | <char_type> | <boolean_type> | <scoped_name>` and
/// admits no bitmask, so no `.idl` file reaches this rule and it went
/// unexercised for as long as IDL was the only front end. DDS-XTypes 1.3
/// §7.5.2.6 admits a bitmask wherever it admits an enumeration for a
/// discriminator, and the OMG `dds-xtypes` interoperability suite
/// declares two such unions — `union_bitmask16` over `bitmask B_16` and
/// `union_bitmask32` over `bitmask B_32`, both with plain integral
/// labels — which `zerodds-xml-compose` reads.
///
/// Before this was admitted, the invariant reported every such union as
/// `IncompatibleCaseLabel` ("discriminator is neither integral, boolean,
/// character nor an enum"), the transaction rolled back, and the union
/// reached the wire with no branches at all.
#[test]
fn a_bitmask_discriminator_carries_integral_labels() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "L", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("L");
    let mask = ast
        .add_type(root, "B", TypeKind::Bitmask { bits: Vec::new() })
        .expect("B");
    let _ = ast.add_bit_value(mask, "F1", Some(0)).expect("F1");
    let u = ast
        .add_type(
            root,
            "Value",
            TypeKind::Union {
                discriminator: Ref::Resolved(mask),
                cases: Vec::new(),
                default_case: None,
            },
        )
        .expect("Value");
    let label = ast.add_int_expr(2).expect("2");

    let mut tx = ast.begin();
    let _ = tx
        .ast()
        .add_case(
            u,
            vec![zerodds_ast::CaseLabel::Value(label)],
            MemberSpec::new("a", Ref::Resolved(long)),
        )
        .expect("case");
    tx.commit()
        .expect("a bitmask discriminator takes an integral label");
    assert_eq!(ast.cases_of(u).len(), 1, "the case is in the model");
}

#[test]
fn the_same_label_is_accepted_while_the_discriminator_is_still_pending() {
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
                discriminator: Ref::Pending(zerodds_ast_core::PendingRef::ScopedName(
                    zerodds_ast_core::ScopedName::absolute(vec!["Disc".to_string()]),
                )),
                cases: Vec::new(),
                default_case: None,
            },
        )
        .expect("Value");
    let label = ast.add_int_expr(1).expect("1");

    let mut tx = ast.begin();
    let _ = tx
        .ast()
        .add_case(
            u,
            vec![zerodds_ast::CaseLabel::Value(label)],
            MemberSpec::new("a", Ref::Resolved(long)),
        )
        .expect("case");
    // The check cannot be performed yet, and guessing would be worse
    // than deferring.
    assert!(tx.commit().is_ok());
}

#[test]
fn every_violation_is_reported_rather_than_only_the_first() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "L", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("L");
    let a = ast
        .add_type(
            root,
            "A",
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("A");
    let b = ast
        .add_type(
            root,
            "B",
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("B");

    let mut tx = ast.begin();
    for aggregate in [a, b] {
        let _ = tx
            .ast()
            .add_member(aggregate, MemberSpec::new("x", Ref::Resolved(long)))
            .expect("first x");
        let _ = tx
            .ast()
            .add_member(aggregate, MemberSpec::new("x", Ref::Resolved(long)))
            .expect("second x");
    }
    let Err(AstError::Invariant(violations)) = tx.commit() else {
        panic!("expected rejection");
    };
    assert!(
        violations.len() >= 2,
        "expected both aggregates reported, got {violations:?}"
    );
}

#[test]
fn removing_a_part_takes_its_handle_out_of_the_list_that_held_it() {
    // `remove` promises to cascade to the parts a node owns and to
    // detach it from its parent. Only types, members, cases,
    // enumerators, bit values, bitfields and modules did: an operation,
    // a parameter, an attribute, a constant, an exception and an
    // annotation declaration were taken out of the arena and left in the
    // list that named them. The arena is generational, so the stale
    // handle answered `Err` instead of the wrong node, and no test could
    // see the phantom entry.
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "long", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("long");
    let interface = ast
        .add_interface(root, "I", zerodds_ast::InterfaceKind::Interface, Vec::new())
        .expect("I");
    let operation = ast
        .add_operation(interface, "ping", None, false)
        .expect("ping");
    let param = ast
        .add_param(
            operation,
            "a",
            zerodds_ast::ParamDirection::In,
            Ref::Resolved(long),
        )
        .expect("a");
    let attribute = ast
        .add_attribute(interface, "rate", Ref::Resolved(long), false)
        .expect("rate");

    let param_node = ast.param(param).expect("a").node_id;
    ast.remove(zerodds_ast::AnyId::Param(param, param_node))
        .expect("remove the parameter");
    assert!(
        ast.operation(operation).expect("ping").params.is_empty(),
        "the parameter's handle stayed in the operation"
    );

    let attribute_node = ast.attribute(attribute).expect("rate").node_id;
    ast.remove(zerodds_ast::AnyId::Attribute(attribute, attribute_node))
        .expect("remove the attribute");
    assert!(
        ast.interface(interface).expect("I").attributes.is_empty(),
        "the attribute's handle stayed in the interface"
    );

    let operation_node = ast.operation(operation).expect("ping").node_id;
    ast.remove(zerodds_ast::AnyId::Operation(operation, operation_node))
        .expect("remove the operation");
    assert!(
        ast.interface(interface).expect("I").operations.is_empty(),
        "the operation's handle stayed in the interface"
    );

    let interface_node = ast.interface(interface).expect("I").node_id;
    ast.remove(zerodds_ast::AnyId::Interface(interface, interface_node))
        .expect("remove the interface");
    assert!(
        !ast.module(root)
            .expect("root")
            .children
            .iter()
            .any(|child| child.node_id() == interface_node),
        "the interface's handle stayed in its module"
    );
}

#[test]
fn removing_an_interface_takes_its_operations_and_attributes_with_it() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "long", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("long");
    let interface = ast
        .add_interface(root, "I", zerodds_ast::InterfaceKind::Interface, Vec::new())
        .expect("I");
    let operation = ast
        .add_operation(interface, "ping", None, false)
        .expect("ping");
    let param = ast
        .add_param(
            operation,
            "a",
            zerodds_ast::ParamDirection::In,
            Ref::Resolved(long),
        )
        .expect("a");
    let attribute = ast
        .add_attribute(interface, "rate", Ref::Resolved(long), false)
        .expect("rate");
    let interface_node = ast.interface(interface).expect("I").node_id;
    let operation_node = ast.operation(operation).expect("ping").node_id;
    let param_node = ast.param(param).expect("a").node_id;
    let attribute_node = ast.attribute(attribute).expect("rate").node_id;

    ast.remove(zerodds_ast::AnyId::Interface(interface, interface_node))
        .expect("remove");

    for (what, node) in [
        ("the operation", operation_node),
        ("the parameter", param_node),
        ("the attribute", attribute_node),
    ] {
        assert!(
            ast.resolve_node_id(node).is_none(),
            "{what} outlived the interface that owned it"
        );
    }
}

/// Two names in one scope that differ only in case are refused.
///
/// OMG IDL 4.2 §7.2.3. The rule reaches an aggregate's own name because
/// the same paragraph puts a declaration into the scope it opens, so
/// `struct Point { long point; }` collides.
///
/// Enforced rather than tolerated because tolerance is not ours to
/// extend: `tao_idl` and Cyclone's `idlc` both reject such a file, so a
/// type we alone accept cannot be described to a peer. Three of our own
/// fixtures turned out to be written this way and were measuring through
/// IDL no other compiler would take.
#[test]
fn two_names_in_one_scope_differing_only_in_case_are_refused() {
    let (mut ast, long, s) = model_with_struct();
    let before = ast.node_count();

    let mut tx = ast.begin();
    let _ = tx
        .ast()
        .add_member(s, MemberSpec::new("point", Ref::Resolved(long)))
        .expect("member");
    let outcome = tx.commit();

    let Err(AstError::Invariant(violations)) = outcome else {
        panic!("`Point` and `point` share a scope and must not commit");
    };
    assert!(
        violations
            .iter()
            .any(|v| matches!(v, InvariantViolation::NameCollidesOnlyInCase { .. })),
        "{violations:?}"
    );
    assert_eq!(ast.node_count(), before);
}

/// A name that differs in more than case is untouched by the rule.
///
/// The guard against reading §7.2.3 as "no member may resemble its
/// aggregate": `Point` and `points` differ by a letter, not by case, and
/// the model takes it.
#[test]
fn a_name_differing_by_more_than_case_still_commits() {
    let (mut ast, long, s) = model_with_struct();

    let mut tx = ast.begin();
    let _ = tx
        .ast()
        .add_member(s, MemberSpec::new("points", Ref::Resolved(long)))
        .expect("member");
    tx.commit()
        .expect("`Point` and `points` are distinct names");

    assert_eq!(ast.type_(s).expect("Point").kind.members().len(), 1);
}
