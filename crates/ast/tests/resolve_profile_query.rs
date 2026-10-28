// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Resolution, profile validation, query and snapshot.
//! Spec §2, §6, §8.2, §9.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::{Ast, Bound, CharWidth, MemberSpec, PrimitiveKind, Profile, TypeKind};
use zerodds_ast_core::{BreakCause, IndexLookup, PendingRef, Ref, ScopedName};

fn scoped(segments: &[&str]) -> ScopedName {
    ScopedName::absolute(segments.iter().map(|s| (*s).to_string()).collect())
}

fn pending_key(segments: &[&str]) -> PendingRef {
    PendingRef::ScopedName(scoped(segments))
}

// -- resolution, spec §2 -------------------------------------------------

#[test]
fn a_type_arriving_later_resolves_every_waiter_in_one_step() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let robot = ast.ensure_module(root, "Robot").expect("Robot");

    // Three members reference ::Robot::Pose before it exists.
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
    for name in ["a", "b", "c"] {
        let _ = ast
            .add_member(
                holder,
                MemberSpec::new(name, Ref::Pending(pending_key(&["Robot", "Pose"]))),
            )
            .expect("member");
    }
    assert_eq!(
        ast.pending_waiters(&pending_key(&["Robot", "Pose"])).len(),
        3
    );

    let pose = ast
        .add_type(
            robot,
            "Pose",
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("Pose");
    let resolved = ast
        .resolve_pending(&pending_key(&["Robot", "Pose"]), pose)
        .expect("resolve");

    assert_eq!(resolved, 3);
    for member in ast.members_of(holder) {
        assert!(ast.member(*member).expect("member").type_.is_resolved());
    }
    assert!(
        ast.pending_waiters(&pending_key(&["Robot", "Pose"]))
            .is_empty()
    );
}

#[test]
fn an_unresolvable_name_lands_in_broken_with_not_found() {
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
    let m = ast
        .add_member(
            holder,
            MemberSpec::new("x", Ref::Pending(pending_key(&["Nope"]))),
        )
        .expect("x");

    let report = ast.resolve_all_by_name().expect("resolve pass");
    assert_eq!(report.resolved, 0);
    assert!(!report.is_clean());
    assert!(
        report
            .broken
            .iter()
            .any(|(_, cause)| matches!(cause, BreakCause::NotFound { .. }))
    );

    // Broken, not still pending.
    let reference = &ast.member(m).expect("x").type_;
    assert!(reference.is_broken());
    assert!(!reference.is_pending());
}

#[test]
fn an_ambiguous_name_reports_every_candidate() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    // Two types of the same scoped name, which a merge of two
    // translation units can produce.
    let _ = ast
        .add_type(root, "Dup", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("first");
    let _ = ast
        .add_type(root, "Dup", TypeKind::Primitive(PrimitiveKind::Int64))
        .expect("second");
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
    let _ = ast
        .add_member(
            holder,
            MemberSpec::new("x", Ref::Pending(pending_key(&["Dup"]))),
        )
        .expect("x");

    let report = ast.resolve_all_by_name().expect("resolve pass");
    let (_, cause) = report.broken.first().expect("a broken reference");
    let BreakCause::Ambiguous { candidates, .. } = cause else {
        panic!("expected ambiguity, got {cause:?}");
    };
    assert_eq!(candidates.len(), 2);
}

#[test]
fn a_broken_reference_is_not_retried_by_a_later_pass() {
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
    let m = ast
        .add_member(
            holder,
            MemberSpec::new("x", Ref::Pending(pending_key(&["Late"]))),
        )
        .expect("x");

    let _ = ast.resolve_all_by_name().expect("first pass");
    assert!(ast.member(m).expect("x").type_.is_broken());

    // The name arrives afterwards; a second pass must not quietly
    // resurrect the reference, because the failure was already reported.
    let _ = ast
        .add_type(root, "Late", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("Late");
    let _ = ast.resolve_all_by_name().expect("second pass");
    assert!(ast.member(m).expect("x").type_.is_broken());
}

#[test]
fn a_recursive_sequence_member_is_legal_and_resolves() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let node = ast
        .add_type(
            root,
            "Node",
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("Node");
    let children = ast
        .add_type(
            root,
            "NodeSeq",
            TypeKind::Sequence {
                element: Ref::Pending(pending_key(&["Node"])),
                bound: Bound::Unbounded,
            },
        )
        .expect("NodeSeq");
    let _ = ast
        .add_member(node, MemberSpec::new("children", Ref::Resolved(children)))
        .expect("children");

    let report = ast.resolve_all_by_name().expect("resolve");
    assert_eq!(report.resolved, 1);
    assert!(report.is_clean());

    // Recursion through a sequence is legal; traversal still terminates.
    let any = ast.any_of_type(node).expect("any");
    let reachable = ast.descendants(any);
    assert!(reachable.len() < 100);
}

#[test]
fn resolution_emits_an_updated_change_carrying_the_transition() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use zerodds_ast_core::{AstObserver, Change, Changeset, FieldValue, RefState};

    struct Watcher {
        updates: AtomicUsize,
    }
    impl AstObserver for Watcher {
        fn on_commit(&self, changeset: &Changeset) {
            for change in changeset.iter() {
                if let Change::Updated { before, after, .. } = change {
                    assert_eq!(*before, FieldValue::Reference(RefState::Pending));
                    assert!(matches!(
                        after,
                        FieldValue::Reference(RefState::Resolved(_))
                    ));
                    self.updates.fetch_add(1, Ordering::SeqCst);
                }
            }
        }
    }

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
    let _ = ast
        .add_member(
            holder,
            MemberSpec::new("x", Ref::Pending(pending_key(&["Target"]))),
        )
        .expect("x");
    let target = ast
        .add_type(root, "Target", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("Target");

    let watcher = Arc::new(Watcher {
        updates: AtomicUsize::new(0),
    });
    let _ = ast.observers_mut().register(watcher.clone());
    let _ = ast
        .resolve_pending(&pending_key(&["Target"]), target)
        .expect("resolve");

    assert_eq!(watcher.updates.load(Ordering::SeqCst), 1);
}

// -- query, spec §6 ------------------------------------------------------

#[test]
fn a_nested_type_is_found_by_its_full_scoped_name() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let robot = ast.ensure_module(root, "Robot").expect("Robot");
    let sensors = ast.ensure_module(robot, "Sensors").expect("Sensors");
    let pose = ast
        .add_type(sensors, "Pose", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("Pose");

    let found = ast
        .find_by_scoped_name(&scoped(&["Robot", "Sensors", "Pose"]))
        .expect("found");
    assert_eq!(found.as_type(), Some(pose));
}

#[test]
fn scoped_name_of_round_trips_through_find_for_every_type() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let robot = ast.ensure_module(root, "Robot").expect("Robot");
    for (module, name) in [(root, "A"), (robot, "B"), (robot, "C")] {
        let _ = ast
            .add_type(module, name, TypeKind::Primitive(PrimitiveKind::Int32))
            .expect("type");
    }

    let types: Vec<_> = ast.types().map(|(id, _)| id).collect();
    for id in types {
        let any = ast.any_of_type(id).expect("any");
        let name = ast.scoped_name_of(any).expect("name");
        assert_eq!(ast.find_by_scoped_name(&name).expect("found"), any);
    }
}

#[test]
fn deleting_a_type_removes_it_from_the_index_in_the_same_commit() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let id = ast
        .add_type(root, "Gone", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("Gone");
    assert!(ast.find_by_scoped_name(&scoped(&["Gone"])).is_ok());

    let any = ast.any_of_type(id).expect("any");
    ast.remove(any).expect("remove");

    // No explicit reindex call in between.
    assert_eq!(
        ast.find_by_scoped_name(&scoped(&["Gone"])),
        Err(IndexLookup::Missing)
    );
}

#[test]
fn a_duplicate_scoped_name_is_reported_as_ambiguous_with_both_nodes() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let _ = ast
        .add_type(root, "Dup", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("first");
    let _ = ast
        .add_type(root, "Dup", TypeKind::Primitive(PrimitiveKind::Int64))
        .expect("second");

    let Err(IndexLookup::Ambiguous(candidates)) = ast.find_by_scoped_name(&scoped(&["Dup"])) else {
        panic!("expected an ambiguity");
    };
    assert_eq!(candidates.len(), 2);
}

#[test]
fn descendants_of_a_recursive_type_terminate_with_the_type_appearing_once() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let node = ast
        .add_type(
            root,
            "Node",
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("Node");
    let seq = ast
        .add_type(
            root,
            "NodeSeq",
            TypeKind::Sequence {
                element: Ref::Resolved(node),
                bound: Bound::Unbounded,
            },
        )
        .expect("NodeSeq");
    let _ = ast
        .add_member(node, MemberSpec::new("children", Ref::Resolved(seq)))
        .expect("children");

    let any = ast.any_of_type(node).expect("any");
    let all = ast.descendants(any);
    let node_appearances = all
        .iter()
        .filter(|entry| entry.as_type() == Some(node))
        .count();
    assert_eq!(node_appearances, 1);
}

#[test]
fn a_ros_name_and_a_repository_id_are_both_indexed() {
    use zerodds_ast_core::{RepositoryId, RosInterfaceKind, RosTypeName};

    let mut ast = Ast::new();
    let root = ast.root_module();
    let pose = ast
        .add_type(root, "Pose", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("Pose");
    let any = ast.any_of_type(pose).expect("any");
    let ros = RosTypeName {
        package: "geometry_msgs".to_string(),
        kind: RosInterfaceKind::Msg,
        name: "Pose".to_string(),
    };
    let repo = RepositoryId("IDL:Pose:1.0".to_string());
    {
        let facets = ast.facets_mut(any).expect("facets");
        facets.ros2_mut().ros_name = Some(ros.clone());
        facets.corba_mut().repository_id = Some(repo.clone());
    }
    ast.reindex();

    assert_eq!(ast.find_by_ros_name(&ros).expect("ros"), pose);
    assert_eq!(ast.find_by_repository_id(&repo).expect("repo"), any);
}

// -- profiles, spec §8.2 -------------------------------------------------

#[test]
fn a_map_is_valid_under_xtypes_and_invalid_under_corba_33() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "L", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("L");
    let _ = ast
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

    // The same model, two profiles, two answers. That asymmetry is the
    // whole point of profile validation.
    assert!(ast.validate(Profile::XTypes13).is_ok());
    assert!(ast.validate(Profile::Corba33).is_err());
}

#[test]
fn a_violation_names_its_rule_and_its_specification_paragraph() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "L", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("L");
    let _ = ast
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

    let Err(violations) = ast.validate(Profile::Corba33) else {
        panic!("expected a violation");
    };
    let first = violations.first().expect("one violation");
    assert!(!first.rule.is_empty());
    assert!(!first.spec_reference.is_empty());
    assert_eq!(first.profile, Profile::Corba33);
}

#[test]
fn ros_2_rejects_a_union_and_a_wide_string() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "L", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("L");
    let _ = ast
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
    let _ = ast
        .add_type(
            root,
            "WideName",
            TypeKind::String {
                width: CharWidth::WChar,
                bound: Bound::Unbounded,
            },
        )
        .expect("WideName");

    let Err(violations) = ast.validate(Profile::Ros2) else {
        panic!("expected violations");
    };
    assert!(violations.len() >= 2);
}

#[test]
fn a_bit_position_beyond_the_bound_is_rejected_by_xtypes() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let mask = ast
        .add_type(root, "Flags", TypeKind::Bitmask { bits: Vec::new() })
        .expect("Flags");
    let any = ast.any_of_type(mask).expect("any");
    ast.facets_mut(any).expect("facets").xtypes_mut().bit_bound = Some(8);
    let _ = ast.add_bit_value(mask, "TOO_HIGH", Some(9)).expect("bit");

    assert!(ast.validate(Profile::XTypes13).is_err());
}

#[test]
fn a_plain_flatdata_type_may_not_reach_a_string() {
    let mut ast = Ast::new();
    let root = ast.root_module();
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
    let s = ast
        .add_type(
            root,
            "Sample",
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("Sample");
    let _ = ast
        .add_member(s, MemberSpec::new("name", Ref::Resolved(text)))
        .expect("name");
    let any = ast.any_of_type(s).expect("any");
    ast.facets_mut(any).expect("facets").flatdata_mut().is_plain = true;

    assert!(ast.validate(Profile::FlatData).is_err());
}

#[test]
fn every_profile_accepts_a_model_of_plain_primitives() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let _ = ast
        .add_type(root, "Score", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("Score");

    for profile in Profile::ALL {
        assert!(
            ast.validate(profile).is_ok(),
            "{profile:?} rejected a model of one int32"
        );
    }
}

#[test]
fn a_pending_reference_fails_completeness_but_passed_the_commit_gate() {
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
    // Committed cleanly — partial knowledge is legal, spec §2.
    let _ = ast
        .add_member(
            holder,
            MemberSpec::new("x", Ref::Pending(pending_key(&["Absent"]))),
        )
        .expect("x");

    // And is still incomplete, which is a different question.
    assert!(ast.validate_complete().is_err());
}

#[test]
fn a_forward_declaration_without_a_body_fails_completeness() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let _ = ast
        .add_forward_type(
            root,
            "Pose",
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("forward");
    assert!(ast.validate_complete().is_err());
}

// -- snapshot, spec §9 ---------------------------------------------------

fn populated_model() -> Ast {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let robot = ast.ensure_module(root, "Robot").expect("Robot");
    let long = ast
        .add_type(robot, "L", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("L");
    let bound = ast.add_int_expr(64).expect("64");
    let _ = ast
        .add_type(
            robot,
            "Name",
            TypeKind::String {
                width: CharWidth::Char,
                bound: Bound::Const(bound),
            },
        )
        .expect("Name");
    let pose = ast
        .add_type(
            robot,
            "Pose",
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("Pose");
    let _ = ast
        .add_member(pose, MemberSpec::new("x", Ref::Resolved(long)))
        .expect("x");
    let _ = ast
        .add_member(pose, MemberSpec::new("y", Ref::Resolved(long)))
        .expect("y");
    let _ = ast
        .add_type(
            robot,
            "PoseSeq",
            TypeKind::Sequence {
                element: Ref::Resolved(pose),
                bound: Bound::Unbounded,
            },
        )
        .expect("PoseSeq");
    ast
}

#[test]
fn a_populated_model_round_trips_preserving_every_node_id() {
    let ast = populated_model();
    let bytes = ast.to_snapshot().expect("snapshot");
    let loaded = Ast::from_snapshot(&bytes).expect("load");

    let original: Vec<_> = ast
        .types()
        .map(|(_, t)| (t.node_id, t.name.clone()))
        .collect();
    let restored: Vec<_> = loaded
        .types()
        .map(|(_, t)| (t.node_id, t.name.clone()))
        .collect();
    assert_eq!(original, restored);
}

#[test]
fn re_serialising_a_loaded_model_is_byte_identical() {
    let ast = populated_model();
    let first = ast.to_snapshot().expect("snapshot");
    let loaded = Ast::from_snapshot(&first).expect("load");
    let second = loaded.to_snapshot().expect("re-snapshot");
    assert_eq!(first, second);
}

#[test]
fn two_independently_built_identical_models_snapshot_identically() {
    // This is what lets a snapshot hash key a build cache.
    let a = populated_model().to_snapshot().expect("a");
    let b = populated_model().to_snapshot().expect("b");
    assert_eq!(a, b);
}

#[test]
fn editing_a_loaded_model_cannot_mint_a_duplicate_identity() {
    let ast = populated_model();
    let bytes = ast.to_snapshot().expect("snapshot");
    let mut loaded = Ast::from_snapshot(&bytes).expect("load");

    let highest = loaded
        .types()
        .map(|(_, t)| t.node_id)
        .max()
        .expect("at least one type");
    let root = loaded
        .all_nodes()
        .find_map(|any| any.as_module())
        .expect("a module");
    let fresh = loaded
        .add_type(root, "Fresh", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("Fresh");
    assert!(loaded.type_(fresh).expect("Fresh").node_id > highest);
}

#[test]
fn indices_are_usable_immediately_after_loading() {
    let ast = populated_model();
    let bytes = ast.to_snapshot().expect("snapshot");
    let loaded = Ast::from_snapshot(&bytes).expect("load");
    assert!(
        loaded
            .find_by_scoped_name(&scoped(&["Robot", "Pose"]))
            .is_ok()
    );
}

#[test]
fn a_truncated_snapshot_is_rejected() {
    let ast = populated_model();
    let mut bytes = ast.to_snapshot().expect("snapshot");
    bytes.truncate(bytes.len() / 2);
    assert!(Ast::from_snapshot(&bytes).is_err());
}

#[test]
fn a_snapshot_with_a_wrong_magic_is_rejected() {
    let ast = populated_model();
    let mut bytes = ast.to_snapshot().expect("snapshot");
    bytes[0] = b'X';
    assert!(Ast::from_snapshot(&bytes).is_err());
}

#[test]
fn a_snapshot_with_an_unknown_version_is_rejected() {
    let ast = populated_model();
    let mut bytes = ast.to_snapshot().expect("snapshot");
    // Version sits right behind the eight magic bytes.
    bytes[8] = 0xff;
    bytes[9] = 0xff;
    assert!(Ast::from_snapshot(&bytes).is_err());
}

#[test]
fn trailing_bytes_are_rejected() {
    let ast = populated_model();
    let mut bytes = ast.to_snapshot().expect("snapshot");
    bytes.push(0);
    assert!(Ast::from_snapshot(&bytes).is_err());
}

#[test]
fn an_empty_buffer_is_rejected() {
    assert!(Ast::from_snapshot(&[]).is_err());
}

#[test]
fn a_snapshot_declaring_more_elements_than_the_buffer_holds_is_rejected() {
    // Found by the fuzz target as an out-of-memory abort: the loader
    // reserved for the declared count before discovering the bytes were
    // not there. The count is now bounded by the remaining buffer.
    let ast = populated_model();
    let mut bytes = ast.to_snapshot().expect("snapshot");
    // The first section's slot capacity sits behind magic(8) +
    // version(2) + four u64 high-water marks.
    let at = 8 + 2 + 8 * 4;
    bytes[at..at + 4].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(Ast::from_snapshot(&bytes).is_err());
}

#[test]
fn a_snapshot_naming_a_slot_beyond_its_capacity_is_rejected() {
    // The second out-of-memory path: a huge slot index made the arena
    // grow its slot vector to that index before failing.
    let ast = populated_model();
    let mut bytes = ast.to_snapshot().expect("snapshot");
    // First module entry's slot, behind the header, the high-water
    // marks, the slot capacity and the element count.
    let at = 8 + 2 + 8 * 4 + 4 + 4;
    bytes[at..at + 4].copy_from_slice(&(u32::MAX - 1).to_be_bytes());
    assert!(Ast::from_snapshot(&bytes).is_err());
}

// -- snapshot completeness, spec §9 --------------------------------------

/// A model holding at least one node of every kind the model can hold.
fn model_with_every_node_kind() -> Ast {
    use zerodds_ast::{
        CaseLabel, EndpointRole, InterfaceKind, ParamDirection, QosPolicy, ValuetypeKind,
    };

    let mut ast = Ast::new();
    let root = ast.root_module();
    let module = ast.ensure_module(root, "Robot").expect("Robot");

    let long = ast
        .add_type(module, "L", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("L");
    let expr = ast.add_int_expr(7).expect("7");
    let _ = ast
        .add_const(module, "MAX", Ref::Resolved(long), expr)
        .expect("MAX");

    let s = ast
        .add_type(
            module,
            "Pose",
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("Pose");
    let _ = ast
        .add_member(s, MemberSpec::new("x", Ref::Resolved(long)))
        .expect("x");

    let u = ast
        .add_type(
            module,
            "Value",
            TypeKind::Union {
                discriminator: Ref::Resolved(long),
                cases: Vec::new(),
                default_case: None,
            },
        )
        .expect("Value");
    let _ = ast
        .add_case(
            u,
            vec![CaseLabel::Default],
            MemberSpec::new("v", Ref::Resolved(long)),
        )
        .expect("case");

    let e = ast
        .add_type(
            module,
            "Colour",
            TypeKind::Enumeration {
                enumerators: Vec::new(),
            },
        )
        .expect("Colour");
    let _ = ast.add_enumerator(e, "RED", None).expect("RED");

    let b = ast
        .add_type(module, "Flags", TypeKind::Bitmask { bits: Vec::new() })
        .expect("Flags");
    let _ = ast.add_bit_value(b, "A", Some(3)).expect("A");

    let bs = ast
        .add_type(
            module,
            "Header",
            TypeKind::Bitset {
                base: None,
                fields: Vec::new(),
            },
        )
        .expect("Header");
    let _ = ast.add_bitfield(bs, Some("flag"), 2, None).expect("flag");

    let _ = ast
        .add_type(
            module,
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

    let ex = ast.add_exception(module, "Failure").expect("Failure");
    let _ = ast
        .add_exception_member(ex, MemberSpec::new("code", Ref::Resolved(long)))
        .expect("code");

    let iface = ast
        .add_interface(module, "Control", InterfaceKind::Local, Vec::new())
        .expect("Control");
    let op = ast
        .add_operation(iface, "move", Some(Ref::Resolved(long)), true)
        .expect("move");
    let _ = ast
        .add_param(op, "d", ParamDirection::InOut, Ref::Resolved(long))
        .expect("d");
    let _ = ast
        .add_attribute(iface, "speed", Ref::Resolved(long), true)
        .expect("speed");

    let ann = ast.add_annotation_def(module, "my_ann").expect("@my_ann");
    let _ = ast
        .add_annotation_member(ann, "value", Ref::Resolved(long), Some(expr))
        .expect("value");

    let lib = ast.add_library("Deployment").expect("library");
    let profile = ast
        .add_qos_profile(
            lib,
            "Reliable",
            None,
            vec![QosPolicy::Reliability {
                kind: None,
                max_blocking_time: None,
            }],
        )
        .expect("profile");
    let domain = ast.add_domain(lib, "Main", 3).expect("domain");
    let topic = ast
        .add_topic(
            domain,
            "RobotPose",
            Ref::Resolved(s),
            Some(Ref::Resolved(profile)),
        )
        .expect("topic");
    let participant = ast
        .add_participant(lib, "P", Ref::Resolved(domain), None)
        .expect("participant");
    let _ = ast
        .add_endpoint(
            participant,
            "w",
            EndpointRole::DataWriter,
            Ref::Resolved(topic),
            None,
        )
        .expect("writer");

    ast
}

#[test]
fn every_node_kind_round_trips_through_the_snapshot() {
    use std::collections::BTreeSet;
    use zerodds_ast::NodeKind;

    let ast = model_with_every_node_kind();
    let present: BTreeSet<NodeKind> = ast.all_nodes().map(zerodds_ast::AnyId::kind).collect();
    // The fixture is only meaningful if it really holds every kind.
    for kind in NodeKind::ALL {
        assert!(present.contains(&kind), "the fixture lacks a {kind:?}");
    }

    let bytes = ast.to_snapshot().expect("snapshot");
    let loaded = Ast::from_snapshot(&bytes).expect("load");

    let restored: BTreeSet<NodeKind> = loaded.all_nodes().map(zerodds_ast::AnyId::kind).collect();
    assert_eq!(present, restored, "a node kind was lost in the round trip");
    assert_eq!(ast.node_count(), loaded.node_count());
}

#[test]
fn a_model_of_every_node_kind_re_serialises_byte_identically() {
    let ast = model_with_every_node_kind();
    let first = ast.to_snapshot().expect("snapshot");
    let loaded = Ast::from_snapshot(&first).expect("load");
    assert_eq!(loaded.to_snapshot().expect("second"), first);
}

#[test]
fn the_facet_payload_survives_the_round_trip() {
    use zerodds_ast::{Extensibility, MemberIdAssignment, MemberIdOrigin, TryConstruct};

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
    let any = ast.any_of_member(member).expect("any");
    {
        let facets = ast.facets_mut(any).expect("facets");
        let x = facets.xtypes_mut();
        x.extensibility = Some(Extensibility::Mutable);
        x.is_key = true;
        x.is_must_understand = true;
        x.try_construct = Some(TryConstruct::Trim);
        x.bit_bound = Some(16);
        x.unit = Some("m/s".to_string());
        x.member_id = Some(MemberIdAssignment {
            effective: 42,
            origin: MemberIdOrigin::Hashed {
                source_name: "x".to_string(),
            },
        });
        facets.corba_mut().repository_id = Some(zerodds_ast_core::RepositoryId(
            "IDL:Carrier/x:1.0".to_string(),
        ));
        facets.flatdata_mut().is_plain = true;
        facets.vendor.rti = Some(zerodds_ast::RtiFacet::default());
    }

    let bytes = ast.to_snapshot().expect("snapshot");
    let loaded = Ast::from_snapshot(&bytes).expect("load");

    let node_id = ast.member(member).expect("x").node_id;
    let restored_any = loaded.resolve_node_id(node_id).expect("member");
    let facets = loaded.facets(restored_any).expect("facets");
    let x = facets.xtypes.as_ref().expect("xtypes");

    assert_eq!(x.extensibility, Some(Extensibility::Mutable));
    assert!(x.is_key);
    assert!(x.is_must_understand);
    assert_eq!(x.try_construct, Some(TryConstruct::Trim));
    assert_eq!(x.bit_bound, Some(16));
    assert_eq!(x.unit.as_deref(), Some("m/s"));
    let assignment = x.member_id.as_ref().expect("member id");
    assert_eq!(assignment.effective, 42);
    assert!(matches!(assignment.origin, MemberIdOrigin::Hashed { .. }));
    assert_eq!(
        facets
            .corba
            .as_ref()
            .and_then(|c| c.repository_id.as_ref())
            .map(|id| id.0.as_str()),
        Some("IDL:Carrier/x:1.0")
    );
    assert!(facets.flatdata.as_ref().is_some_and(|f| f.is_plain));
    assert!(facets.vendor.rti.is_some());
}

// -- completing a forward declaration, spec §3.2 -------------------------

/// A model holding `Holder { Pose value; }` where `Pose` exists only as
/// a forward declaration.
fn forward_and_waiter() -> (
    Ast,
    zerodds_ast_core::Id<zerodds_ast::TypeNode>,
    zerodds_ast_core::Id<zerodds_ast::MemberNode>,
) {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let pose = ast
        .add_forward_type(
            root,
            "Pose",
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("forward Pose");
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
        .add_member(holder, MemberSpec::new("value", Ref::Resolved(pose)))
        .expect("value");
    (ast, pose, member)
}

#[test]
fn completing_a_forward_type_keeps_the_handle_every_reference_holds() {
    let (mut ast, pose, member) = forward_and_waiter();
    assert_eq!(
        ast.type_(pose).expect("Pose").completeness,
        zerodds_ast::Completeness::Forward
    );

    ast.complete_type(
        pose,
        TypeKind::Structure {
            base: None,
            members: Vec::new(),
        },
    )
    .expect("complete");

    assert_eq!(
        ast.type_(pose).expect("Pose").completeness,
        zerodds_ast::Completeness::Complete
    );
    assert_eq!(
        ast.member(member).expect("value").type_.resolved(),
        Some(pose),
        "the reference taken against the forward declaration moved"
    );
}

#[test]
fn completing_a_type_that_already_has_a_body_is_rejected() {
    // Two definitions of one name are a duplicate, not a completion.
    let (mut ast, pose, _) = forward_and_waiter();
    ast.complete_type(
        pose,
        TypeKind::Structure {
            base: None,
            members: Vec::new(),
        },
    )
    .expect("first completion");

    let again = ast.complete_type(
        pose,
        TypeKind::Structure {
            base: None,
            members: Vec::new(),
        },
    );
    assert!(matches!(
        again,
        Err(zerodds_ast::AstError::AlreadyComplete { .. })
    ));
}

#[test]
fn completing_a_type_emits_one_completeness_change() {
    let (mut ast, pose, _) = forward_and_waiter();
    let node_id = ast.type_(pose).expect("Pose").node_id;

    let mut txn = ast.begin();
    txn.ast()
        .complete_type(
            pose,
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("complete");
    let changeset = txn.commit().expect("commit");

    let completeness = changeset
        .iter()
        .filter(|change| {
            matches!(
                change,
                zerodds_ast_core::Change::Updated { node, field, .. }
                    if *node == node_id && *field == zerodds_ast::field::COMPLETENESS
            )
        })
        .count();
    assert_eq!(
        completeness,
        1,
        "{:?}",
        changeset.iter().collect::<Vec<_>>()
    );
}

#[test]
fn completing_a_forward_type_replaces_its_pending_keys() {
    // The old kind's references disappear with it; a waiter entry that
    // outlived them would send a later resolution at a reference that is
    // no longer there.
    let mut ast = Ast::new();
    let root = ast.root_module();
    let alias = ast
        .add_forward_type(
            root,
            "Alias",
            TypeKind::Alias {
                aliased: Ref::Pending(pending_key(&["Gone"])),
            },
        )
        .expect("forward Alias");
    assert_eq!(ast.pending_waiters(&pending_key(&["Gone"])).len(), 1);

    ast.complete_type(
        alias,
        TypeKind::Alias {
            aliased: Ref::Pending(pending_key(&["Arrived"])),
        },
    )
    .expect("complete");

    assert!(
        ast.pending_waiters(&pending_key(&["Gone"])).is_empty(),
        "the replaced reference is still indexed"
    );
    assert_eq!(ast.pending_waiters(&pending_key(&["Arrived"])).len(), 1);
}

// -- rollback of field updates, spec §5.1 --------------------------------

#[test]
fn a_rolled_back_transaction_puts_completeness_back() {
    let (mut ast, pose, _) = forward_and_waiter();

    let mut txn = ast.begin();
    txn.ast()
        .complete_type(
            pose,
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("complete");
    txn.rollback();

    assert_eq!(
        ast.type_(pose).expect("Pose").completeness,
        zerodds_ast::Completeness::Forward,
        "rollback left the type complete"
    );
}

#[test]
fn a_rolled_back_resolution_puts_the_reference_back_on_its_key() {
    // Reversal has to restore the key too: a reference put back to
    // pending on nothing would never resolve again.
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
        .add_member(
            holder,
            MemberSpec::new("value", Ref::Pending(pending_key(&["Pose"]))),
        )
        .expect("value");
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

    let mut txn = ast.begin();
    let resolved = txn
        .ast()
        .resolve_pending(&pending_key(&["Pose"]), pose)
        .expect("resolve");
    assert_eq!(resolved, 1);
    txn.rollback();

    let reference = &ast.member(member).expect("value").type_;
    assert!(
        reference.is_pending(),
        "rollback left the reference resolved"
    );
    assert_eq!(reference.pending_key(), Some(&pending_key(&["Pose"])));
    // And it waits in the index again, so a second attempt still finds it.
    assert_eq!(ast.pending_waiters(&pending_key(&["Pose"])).len(), 1);
    assert_eq!(
        ast.resolve_pending(&pending_key(&["Pose"]), pose)
            .expect("resolve again"),
        1
    );
}

#[test]
fn a_rolled_back_resolution_restores_each_reference_of_a_map() {
    // A map whose key and value waited on the same name resolves both in
    // one step, so the reversal has to name each of them separately.
    let mut ast = Ast::new();
    let root = ast.root_module();
    let map = ast
        .add_type(
            root,
            "Table",
            TypeKind::Map {
                key: Ref::Pending(pending_key(&["Cell"])),
                value: Ref::Pending(pending_key(&["Cell"])),
                bound: Bound::Unbounded,
            },
        )
        .expect("Table");
    let cell = ast
        .add_type(
            root,
            "Cell",
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("Cell");

    let mut txn = ast.begin();
    let _ = txn
        .ast()
        .resolve_pending(&pending_key(&["Cell"]), cell)
        .expect("resolve");
    txn.rollback();

    let kind = &ast.type_(map).expect("Table").kind;
    let TypeKind::Map { key, value, .. } = kind else {
        panic!("expected a map");
    };
    assert!(key.is_pending(), "the map key stayed resolved");
    assert!(value.is_pending(), "the map value stayed resolved");
}

// -- resolution reaches every kind of reference, spec §2 -----------------

#[test]
fn a_constant_named_in_an_expression_resolves_like_any_other_reference() {
    // It did not: only member and type references were ever indexed, so
    // a constant in an array dimension stayed pending forever and the
    // dimension never evaluated.
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "long", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("long");
    let expr = ast
        .add_expr(zerodds_ast::ConstExpr::NameRef(Ref::Pending(pending_key(
            &["N"],
        ))))
        .expect("expression");
    let value = ast.add_int_expr(7).expect("value");
    let _ = ast
        .add_const(root, "N", Ref::Resolved(long), value)
        .expect("N");

    let report = ast.resolve_all_by_name().expect("a resolve pass");
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(ast.eval_integral(expr).expect("evaluates"), 7);
}

#[test]
fn an_exception_named_in_a_raises_clause_resolves() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let iface = ast
        .add_interface(root, "I", zerodds_ast::InterfaceKind::Interface, Vec::new())
        .expect("I");
    let op = ast.add_operation(iface, "go", None, false).expect("go");
    ast.add_raises(op, Ref::Pending(pending_key(&["Failure"])))
        .expect("raises");
    let _ = ast.add_exception(root, "Failure").expect("Failure");

    let report = ast.resolve_all_by_name().expect("a resolve pass");
    assert!(report.is_clean(), "{report:?}");
    assert!(
        ast.operation(op).expect("go").raises[0].is_resolved(),
        "the raises clause stayed pending"
    );
}

#[test]
fn a_base_interface_resolves() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let derived = ast
        .add_interface(
            root,
            "Derived",
            zerodds_ast::InterfaceKind::Interface,
            vec![Ref::Pending(pending_key(&["Base"]))],
        )
        .expect("Derived");
    let _ = ast
        .add_interface(
            root,
            "Base",
            zerodds_ast::InterfaceKind::Interface,
            Vec::new(),
        )
        .expect("Base");

    let report = ast.resolve_all_by_name().expect("a resolve pass");
    assert!(report.is_clean(), "{report:?}");
    assert!(ast.interface(derived).expect("Derived").bases[0].is_resolved());
}

#[test]
fn a_name_that_resolves_to_the_wrong_kind_breaks_rather_than_waits() {
    // A member's type reference pointing at an exception is a mistake,
    // and staying pending would hide it behind "not declared yet".
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
        .add_member(
            holder,
            MemberSpec::new("value", Ref::Pending(pending_key(&["Failure"]))),
        )
        .expect("value");
    let _ = ast.add_exception(root, "Failure").expect("Failure");

    let report = ast.resolve_all_by_name().expect("a resolve pass");
    assert!(!report.is_clean(), "the mismatch was not reported");
    assert!(ast.member(member).expect("value").type_.is_broken());
}
