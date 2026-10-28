// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Structural invariants. Spec §8.1.
//!
//! These run on every commit. Completeness rules — no reference left
//! pending, every forward declaration given a body — belong to profile
//! validation (spec §8.2), not here: a partially discovered model is a
//! legal state, not a broken one.

use crate::model::Ast;
use crate::node::AnyId;
use crate::node::type_::TypeKind;
use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use zerodds_ast_core::NodeId;

/// A structural rule a commit would have broken.
///
/// Exhaustive on purpose: a frontend turning violations into diagnostics
/// has to say something specific about each one, and a catch-all arm
/// would answer a new invariant with whatever the fallback happens to
/// be. Adding a variant is a compile error at every such consumer, which
/// is the point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvariantViolation {
    /// A resolved reference points at a node that is not there.
    DanglingReference {
        /// The referring node.
        from: NodeId,
        /// What it referred to.
        detail: &'static str,
    },
    /// An array dimension evaluates to zero or a negative number.
    NonPositiveArrayDimension {
        /// The array type.
        node: NodeId,
        /// The value the dimension evaluated to.
        value: i128,
    },
    /// A union case label is not type-compatible with the discriminator.
    ///
    /// Only raised when the discriminator is resolved: with a pending
    /// discriminator the check cannot be performed, and guessing would
    /// be worse than deferring.
    IncompatibleCaseLabel {
        /// The case.
        case: NodeId,
        /// The discriminator type.
        discriminator: NodeId,
        /// Why the label does not fit.
        detail: &'static str,
    },
    /// Two members of one aggregate share a name.
    DuplicateMemberName {
        /// The aggregate.
        aggregate: NodeId,
        /// The colliding name.
        name: String,
    },
    /// Two enumerators or bit values of one type share a name.
    DuplicateEnumeratorName {
        /// The enumeration or bitmask.
        type_: NodeId,
        /// The colliding name.
        name: String,
    },
    /// Two members of one aggregate resolve to the same member id.
    ///
    /// Names both offenders, so a diagnostic can point at each
    /// declaration rather than only at the second one.
    DuplicateMemberId {
        /// The aggregate.
        aggregate: NodeId,
        /// The id both members claim.
        id: u32,
        /// The member that claimed it first.
        first: NodeId,
        /// The member that collided with it.
        second: NodeId,
    },
    /// A union declares more than one `default:` case.
    MultipleDefaultCases {
        /// The union.
        union: NodeId,
    },
    /// Two names in one scope differ only in case.
    ///
    /// IDL 4.2 §7.2.3 states it directly: identifiers that differ only in
    /// case collide. The rule reaches an aggregate's own name because
    /// §7.2.3 places a declaration into the scope it opens, so
    /// `struct A { long a; }` puts `A` and `a` in one scope.
    ///
    /// Reported rather than silently accepted because the collision is
    /// not ours to resolve: a peer's compiler rejects the file, and a
    /// type that only we can translate is not interoperable. Measured —
    /// tao_idl and `idlc` both refuse it.
    NameCollidesOnlyInCase {
        /// The scope holding both names.
        scope: NodeId,
        /// The name declared first.
        first: String,
        /// The name that collided with it.
        second: String,
    },
}

impl fmt::Display for InvariantViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DanglingReference { from, detail } => {
                write!(f, "node {from:?} has a dangling reference: {detail}")
            }
            Self::NonPositiveArrayDimension { node, value } => {
                write!(
                    f,
                    "array {node:?} has dimension {value}, which must be positive"
                )
            }
            Self::IncompatibleCaseLabel {
                case,
                discriminator,
                detail,
            } => write!(
                f,
                "case {case:?} does not fit discriminator {discriminator:?}: {detail}"
            ),
            Self::DuplicateMemberName { aggregate, name } => {
                write!(f, "aggregate {aggregate:?} declares `{name}` twice")
            }
            Self::DuplicateEnumeratorName { type_, name } => {
                write!(f, "type {type_:?} declares `{name}` twice")
            }
            Self::DuplicateMemberId {
                aggregate,
                id,
                first,
                second,
            } => write!(
                f,
                "aggregate {aggregate:?}: members {first:?} and {second:?} both claim id {id}"
            ),
            Self::MultipleDefaultCases { union } => {
                write!(f, "union {union:?} declares more than one default case")
            }
            Self::NameCollidesOnlyInCase {
                scope,
                first,
                second,
            } => write!(
                f,
                "scope {scope:?} declares `{first}` and `{second}`, which differ only in case"
            ),
        }
    }
}

impl core::error::Error for InvariantViolation {}

/// Check every structural invariant over the touched nodes.
///
/// Collects every violation rather than stopping at the first, so one
/// commit reports all its problems at once. Spec §8.1.
pub fn check(ast: &Ast, touched: &[NodeId]) -> Vec<InvariantViolation> {
    let mut out = Vec::new();
    let mut seen: BTreeSet<NodeId> = BTreeSet::new();

    for node in touched {
        if !seen.insert(*node) {
            continue;
        }
        let Some(any) = ast.resolve_node_id(*node) else {
            // A node touched and then removed in the same transaction is
            // not a violation; only live nodes carry invariants.
            continue;
        };
        // Uniqueness and id collisions are properties of the enclosing
        // aggregate, not of the part, so every part is checked through
        // its parent. Missing one of these kinds means a whole class of
        // collision goes unreported.
        let subject = match any {
            AnyId::Type(id, _) => Some(id),
            AnyId::Member(id, _) => ast.member(id).ok().and_then(|m| m.parent.as_type()),
            AnyId::Case(id, _) => ast.case(id).ok().map(|c| c.parent),
            AnyId::Enumerator(id, _) => ast.enumerator(id).ok().map(|e| e.parent),
            AnyId::BitValue(id, _) => ast.bit_value(id).ok().map(|b| b.parent),
            AnyId::Bitfield(id, _) => ast.bitfield(id).ok().map(|b| b.parent),
            _ => None,
        };

        if let Some(type_id) = subject {
            if let Ok(node) = ast.type_(type_id) {
                let type_node_id = node.node_id;
                if seen.insert(type_node_id) || any.kind() == crate::NodeKind::Type {
                    check_type(ast, type_id, type_node_id, &mut out);
                }
            }
        }
    }

    // The same aggregate can be reached through several of its parts, so
    // identical violations are collapsed rather than reported per part.
    out.dedup();
    out
}

fn check_type(
    ast: &Ast,
    id: zerodds_ast_core::Id<crate::TypeNode>,
    node_id: NodeId,
    out: &mut Vec<InvariantViolation>,
) {
    let Ok(node) = ast.type_(id) else {
        return;
    };

    // Resolved references must point at live nodes.
    for reference in node.kind.type_refs() {
        if let Some(target) = reference.resolved() {
            if ast.type_(target).is_err() {
                out.push(InvariantViolation::DanglingReference {
                    from: node_id,
                    detail: "referenced type is not live",
                });
            }
        }
    }

    match &node.kind {
        TypeKind::Array { dims, .. } => {
            for dim in dims {
                if let Ok(value) = ast.eval_integral(*dim) {
                    if value <= 0 {
                        out.push(InvariantViolation::NonPositiveArrayDimension {
                            node: node_id,
                            value,
                        });
                    }
                }
                // An unevaluable dimension is not a violation here: it
                // may reach a pending reference, which is legal.
            }
        }
        TypeKind::Structure { members, .. } | TypeKind::Valuetype { state: members, .. } => {
            check_member_names(ast, node_id, &node.name, members, out);
            check_member_ids(ast, node_id, members, out);
        }
        TypeKind::Enumeration { enumerators } => {
            let mut names: Vec<&str> = Vec::new();
            for enumerator in enumerators {
                let Ok(e) = ast.enumerator(*enumerator) else {
                    continue;
                };
                if names.contains(&e.name.as_str()) {
                    out.push(InvariantViolation::DuplicateEnumeratorName {
                        type_: node_id,
                        name: e.name.clone(),
                    });
                } else {
                    names.push(&e.name);
                }
            }
        }
        TypeKind::Bitmask { bits } => {
            let mut names: Vec<&str> = Vec::new();
            for bit in bits {
                let Ok(b) = ast.bit_value(*bit) else {
                    continue;
                };
                if names.contains(&b.name.as_str()) {
                    out.push(InvariantViolation::DuplicateEnumeratorName {
                        type_: node_id,
                        name: b.name.clone(),
                    });
                } else {
                    names.push(&b.name);
                }
            }
        }
        TypeKind::Union {
            discriminator,
            cases,
            ..
        } => {
            let mut defaults = 0usize;
            for case in cases {
                let Ok(c) = ast.case(*case) else {
                    continue;
                };
                defaults += c.labels.iter().filter(|l| l.is_default()).count();
            }
            if defaults > 1 {
                out.push(InvariantViolation::MultipleDefaultCases { union: node_id });
            }

            // A union is a scope (IDL 4.2 §7.5.2), and its own name may not
            // be redefined in it: `union U switch (long) { case 1: long u; }`
            // collides exactly as `struct S { long s; }` does.
            // A case that does not resolve is the dangling reference the
            // check above reports; the loop over the defaults skips it the
            // same way.
            let mut elements = Vec::with_capacity(cases.len());
            for case in cases {
                let Ok(c) = ast.case(*case) else {
                    continue;
                };
                elements.push(c.member);
            }
            check_member_names(ast, node_id, &node.name, &elements, out);

            // Label compatibility is only checkable once the
            // discriminator is resolved. With a pending discriminator
            // the model is legal and the check waits.
            if let Some(disc) = discriminator.resolved() {
                check_case_labels(ast, node_id, disc, cases, out);
            }
        }
        _ => {}
    }
}

fn check_member_names(
    ast: &Ast,
    aggregate: NodeId,
    aggregate_name: &str,
    members: &[zerodds_ast_core::Id<crate::MemberNode>],
    out: &mut Vec<InvariantViolation>,
) {
    // The aggregate's own name is seeded into the scope it opens, per IDL
    // 4.2 §7.2.3 — which is why `struct A { long a; }` is a collision and
    // not merely unusual.
    let mut names: Vec<&str> = Vec::new();
    names.push(aggregate_name);
    for member in members {
        let Ok(m) = ast.member(*member) else {
            continue;
        };
        if names.contains(&m.name.as_str()) {
            out.push(InvariantViolation::DuplicateMemberName {
                aggregate,
                name: m.name.clone(),
            });
        } else if let Some(first) = names.iter().find(|held| held.eq_ignore_ascii_case(&m.name)) {
            out.push(InvariantViolation::NameCollidesOnlyInCase {
                scope: aggregate,
                first: String::from(*first),
                second: m.name.clone(),
            });
        } else {
            names.push(&m.name);
        }
    }
}

fn check_member_ids(
    ast: &Ast,
    aggregate: NodeId,
    members: &[zerodds_ast_core::Id<crate::MemberNode>],
    out: &mut Vec<InvariantViolation>,
) {
    let mut assigned: Vec<(u32, NodeId)> = Vec::new();
    for member in members {
        let Ok(m) = ast.member(*member) else {
            continue;
        };
        let Some(id) = m.facets.effective_member_id() else {
            continue;
        };
        if let Some((_, first)) = assigned.iter().find(|(claimed, _)| *claimed == id) {
            out.push(InvariantViolation::DuplicateMemberId {
                aggregate,
                id,
                first: *first,
                second: m.node_id,
            });
        } else {
            assigned.push((id, m.node_id));
        }
    }
}

/// The type a chain of `typedef`s ends in.
///
/// `None` when the chain does not terminate in a readable type: an
/// unresolved reference, or a `typedef` cycle. The bound is the same one
/// every other reference walk in this crate carries, so a cycle stops
/// here instead of spinning, and is reported by the rule that owns it.
fn resolve_aliases<'a>(ast: &'a Ast, start: &'a crate::TypeNode) -> Option<&'a crate::TypeNode> {
    /// The same bound `query.rs` puts on its reference walks.
    const MAX_ALIAS_DEPTH: usize = 64;
    let mut node = start;
    for _ in 0..MAX_ALIAS_DEPTH {
        let TypeKind::Alias { aliased } = &node.kind else {
            return Some(node);
        };
        node = ast.type_(aliased.resolved()?).ok()?;
    }
    None
}

fn check_case_labels(
    ast: &Ast,
    union: NodeId,
    discriminator: zerodds_ast_core::Id<crate::TypeNode>,
    cases: &[zerodds_ast_core::Id<crate::CaseNode>],
    out: &mut Vec<InvariantViolation>,
) {
    let Ok(disc) = ast.type_(discriminator) else {
        return;
    };
    // The kind is read through the alias chain, not off `disc` directly.
    // IDL 4.2 §7.4.1.4.4.3 rule (50) writes `<switch_type_spec>` as
    // `<integer_type> | <char_type> | <boolean_type> | <scoped_name>`,
    // and a `<scoped_name>` naming a `typedef` is an alias for the type
    // it names (§7.4.1.4.4.5) — so `typedef long L; union U switch (L)`
    // is a legal integral discriminator. Reading `disc.kind` without
    // following the alias made every such union an `IncompatibleCaseLabel`,
    // reported against the very clause that admits it. Found by
    // `tests/interop-corpus/union-discriminator-typedef-enum`.
    let Some(resolved) = resolve_aliases(ast, disc) else {
        // The chain does not terminate in a type this pass can read —
        // an unresolved reference or a `typedef` cycle. Both are other
        // rules' findings, and reporting a label violation on top of
        // them would name the wrong declaration.
        return;
    };
    let integral = matches!(
        &resolved.kind,
        TypeKind::Primitive(p) if p.is_integral()
            || matches!(
                p,
                crate::PrimitiveKind::Boolean
                    | crate::PrimitiveKind::Char
                    | crate::PrimitiveKind::WChar
            )
    ) || matches!(
        &resolved.kind,
        // A bitmask is a discriminator type as well as an enumeration.
        // OMG IDL 4.2's `<switch_type_spec>` (rule 50) has no bitmask, so
        // no `.idl` file can reach this and the rule went unexercised —
        // but DDS-XTypes 1.3 §7.5.2.6 admits a bitmask wherever it
        // admits an enumeration for a union's discriminator, and the
        // OMG's own `dds-xtypes` interoperability suite declares two of
        // them: `union_bitmask16` over `bitmask B_16` and
        // `union_bitmask32` over `bitmask B_32`, both with plain
        // integral labels. Without this arm every such union was
        // reported as `IncompatibleCaseLabel` — "discriminator is
        // neither integral, boolean, character nor an enum" — and the
        // model refused the cases outright, so the union reached the
        // wire with no branches at all.
        TypeKind::Enumeration { .. } | TypeKind::Bitmask { .. }
    );

    if !integral {
        // A non-integral, non-enum, non-bitmask discriminator cannot
        // carry labels at all; DDS-XTypes 1.3 §7.2.2.4.4 restricts the
        // set.
        for case in cases {
            let Ok(c) = ast.case(*case) else {
                continue;
            };
            if c.labels.iter().any(|l| !l.is_default()) {
                out.push(InvariantViolation::IncompatibleCaseLabel {
                    case: c.node_id,
                    discriminator: disc.node_id,
                    detail: "discriminator is neither integral, boolean, character nor an enum",
                });
            }
        }
        return;
    }

    // With an integral discriminator, a label must evaluate; one that
    // cannot is only a violation when the failure is not an unresolved
    // reference.
    for case in cases {
        let Ok(c) = ast.case(*case) else {
            continue;
        };
        for label in &c.labels {
            let Some(expr) = label.expr() else {
                continue;
            };
            if let Err(error) = ast.eval_integral(expr) {
                if !matches!(error, crate::EvalError::Unresolved { .. }) {
                    out.push(InvariantViolation::IncompatibleCaseLabel {
                        case: c.node_id,
                        discriminator: disc.node_id,
                        detail: "label does not evaluate to an integral value",
                    });
                }
            }
        }
    }

    let _ = union;
}
