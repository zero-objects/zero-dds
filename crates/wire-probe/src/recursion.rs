//! The aggregates of a type graph that sit on a cycle.
//!
//! A dump emitter inlines a member's statements into its owner's, and a
//! type graph with a cycle has no finite inline expansion: the CORBA
//! C++ emitter followed `recursive-mutual`'s
//! `A → sequence<B> → B → sequence<A>` until the Rust stack ran out,
//! and the C and Fast DDS emitters shared the walk and the crash. The
//! types this module names are emitted as functions instead, and the
//! expansion calls them where it would have unrolled them.
//!
//! ## Which walks ask, and which two did not
//!
//! Three emitters carry the answer: [`mod@crate::dump`], [`crate::dump_cpp`]
//! and [`crate::corba_cpp`]'s dump half, each of which emits one
//! function per aggregate named here and calls it in place of the
//! expansion.
//!
//! Two ran past it and followed the cycle instead, measured on
//! 2026-08-31 over `recursive-mutual` — both reached
//! "has overflowed its stack" and `SIGABRT`:
//!
//! - `corba_java::dump_at`. The C++ half of that same file was given the
//!   machinery; the Java half was not.
//! - `dump_ros::emit`. Its comment named `ros_idl` as the gate, and
//!   `ros_idl` does refuse a recursive type — but `wireprobe-dump --ros`
//!   is a separate process invocation and no gate on this function.
//!
//! Neither has a shape to emit a function into — a Java method body and
//! a C block inside the ROS decode template — and neither column takes a
//! recursive type at all, so both now refuse one by name, in the words
//! `corba_idl` and `ros_idl` already use for it. A refusal is a cell
//! that says why; a stack overflow is a cell that measured nothing.
//!
//! The fill emitters do not ask, and do not need to: a fill walk follows
//! the sample, and a recursive type has finite samples because the
//! sequence that closes its cycle is eventually empty. That is a
//! termination argument rather than a bound, and it is the sample's
//! nesting that bounds them.

use crate::contract;
use zerodds_ast::{Ast, TypeKind, TypeNode};
use zerodds_ast_core::Id;

/// The aggregates reachable from `root` that sit on a cycle, in
/// first-visit order.
pub fn recursive_aggregates(ast: &Ast, root: Id<TypeNode>) -> Vec<Id<TypeNode>> {
    let mut all: Vec<Id<TypeNode>> = Vec::new();
    let mut queue = Vec::new();
    collect_aggregates(ast, root, &mut queue, &mut Vec::new());
    while let Some(id) = queue.pop() {
        if all.contains(&id) {
            continue;
        }
        all.push(id);
        queue.extend(aggregate_successors(ast, id));
    }
    all.into_iter()
        .filter(|id| reaches(ast, *id, *id, &mut Vec::new()))
        .collect()
}

/// Whether `target` is reachable from `from` over aggregate edges.
///
/// zerodds-lint: recursion-depth = the number of aggregates in the
/// model. `seen` holds every aggregate this descent has already entered
/// and nothing is entered twice, so the depth cannot pass the count of
/// distinct aggregates the model declares — which is what makes the
/// walk end on the cycle it is looking for rather than following it.
fn reaches(
    ast: &Ast,
    from: Id<TypeNode>,
    target: Id<TypeNode>,
    seen: &mut Vec<Id<TypeNode>>,
) -> bool {
    for next in aggregate_successors(ast, from) {
        if next == target {
            return true;
        }
        if !seen.contains(&next) {
            seen.push(next);
            if reaches(ast, next, target, seen) {
                return true;
            }
        }
    }
    false
}

/// The aggregates one step below `type_`: those its members or case
/// members reach without passing through another aggregate.
fn aggregate_successors(ast: &Ast, type_: Id<TypeNode>) -> Vec<Id<TypeNode>> {
    let mut found = Vec::new();
    let Ok(node) = ast.type_(type_) else {
        return found;
    };
    match &node.kind {
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            for member in contract::members(ast, type_) {
                collect_aggregates(ast, member.type_, &mut found, &mut Vec::new());
            }
        }
        TypeKind::Union { .. } => {
            for case in contract::union_cases(ast, type_) {
                collect_aggregates(ast, case.type_, &mut found, &mut Vec::new());
            }
        }
        _ => {}
    }
    found
}

/// The first aggregates below `type_`, walking through the connectors
/// — aliases, sequences, arrays, maps — and stopping at each aggregate
/// found.
///
/// zerodds-lint: recursion-depth = the number of connectors in the
/// model. `open` holds the connectors this descent has already entered
/// and one is never entered twice, so the depth cannot pass the count of
/// distinct typedef, sequence, array and map nodes the model declares.
///
/// The guard is not decoration. A connector chain is a tree in a model
/// written in IDL — a `sequence<sequence<long>>` is two nodes the parser
/// made, and a typedef names a type that was already declared — but a
/// typedef that resolves to itself is a chain with no end, and the
/// frontend admits `typedef Tee Tee;` with no diagnostic. This function
/// is the one every dump emitter asks *before* it can refuse a cycle, so
/// it is the one place that cannot answer by running out of stack.
/// `contract::reject_alias_cycle` refuses the same model at each
/// emitter's entry; this is what holds when the function is called on
/// its own.
fn collect_aggregates(
    ast: &Ast,
    type_: Id<TypeNode>,
    found: &mut Vec<Id<TypeNode>>,
    open: &mut Vec<Id<TypeNode>>,
) {
    let Ok(node) = ast.type_(type_) else {
        return;
    };
    if open.contains(&type_) {
        // A connector already on this descent: following it again adds
        // no aggregate that is not found by the first visit.
        return;
    }
    open.push(type_);
    match &node.kind {
        TypeKind::Alias { aliased } => {
            if let Some(target) = aliased.resolved() {
                collect_aggregates(ast, target, found, open);
            }
        }
        TypeKind::Sequence { element, .. } | TypeKind::Array { element, .. } => {
            if let Some(target) = element.resolved() {
                collect_aggregates(ast, target, found, open);
            }
        }
        TypeKind::Map { key, value, .. } => {
            if let Some(target) = key.resolved() {
                collect_aggregates(ast, target, found, open);
            }
            if let Some(target) = value.resolved() {
                collect_aggregates(ast, target, found, open);
            }
        }
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } | TypeKind::Union { .. } => {
            if !found.contains(&type_) {
                found.push(type_);
            }
        }
        _ => {}
    }
}
