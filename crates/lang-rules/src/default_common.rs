// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What the default emitters of every language column ask of the model
//! before they spell anything: the type an alias chain ends in, and the
//! default value of a union discriminator.

use crate::encode::EncodeError;
use zerodds_ast::{Ast, CaseLabel, MemberNode, TypeKind, TypeNode};
use zerodds_ast_core::Id;

/// How many `typedef` links an alias chain may have before the walk stops.
const MAX_ALIAS_DEPTH: usize = 64;

/// The type `id`'s alias chain ends in, or the last link whose target does
/// not resolve.
///
/// A counted loop: a `typedef` cycle is a finding the model's own invariant
/// pass reports rather than a shape it rejects, and a recursive walk would
/// read such a chain with nothing counting it.
pub(crate) fn resolve_alias(ast: &Ast, id: Id<TypeNode>) -> Id<TypeNode> {
    let mut current = id;
    for _ in 0..MAX_ALIAS_DEPTH {
        match ast.type_(current).map(|node| &node.kind) {
            Ok(TypeKind::Alias { aliased }) => match aliased.resolved() {
                Some(target) => current = target,
                None => return current,
            },
            _ => return current,
        }
    }
    current
}

/// The default value of a union discriminator (IDL 4.2 §7.4.1.4.4.4.2,
/// Table 9): the enumerator marked `@default_literal`, else the first one,
/// as its value; zero for every other discriminator kind.
pub(crate) fn discriminator_default_value(ast: &Ast, discriminator: Id<TypeNode>) -> Option<i128> {
    let discriminator = resolve_alias(ast, discriminator);
    match ast.type_(discriminator).map(|node| node.kind.clone()) {
        Ok(TypeKind::Enumeration { .. }) => {
            // The first literal's *value*, which `@value` may have moved away
            // from zero.
            let mut first = None;
            for enumerator in ast.enumerators_of(discriminator) {
                // `None`, not `continue`. This function answers "the
                // discriminator's default value", and the line below already
                // says `None` when an enumerator's value does not evaluate.
                // Skipping the enumerator instead makes the *next* one the
                // default — a union that selects a different arm than the
                // source declared, with nothing to show for it.
                let Ok(node) = ast.enumerator(*enumerator) else {
                    return None;
                };
                let value = ast.effective_enumerator_value(*enumerator)?;
                if node
                    .facets
                    .xtypes
                    .as_ref()
                    .is_some_and(|x| x.is_default_literal)
                {
                    return Some(value);
                }
                if first.is_none() {
                    first = Some(value);
                }
            }
            first
        }
        // Every other discriminator kind IDL 4.2 §7.4.1.4.4.4.2 allows —
        // integer, `char`, `boolean` — has zero for its Table 9 default, and
        // `'\0'` and `FALSE` are zero.
        Ok(TypeKind::Primitive(_)) => Some(0),
        _ => None,
    }
}

/// The case the discriminator value `selector` selects in union `id`: the
/// case with a label of that value, else the `default:` case (IDL 4.2
/// §7.4.1.4.4.4.2), as its member and the member's type.
///
/// A case or member the model does not hold, and a case type that does not
/// resolve, is an error and not a case to step over: stepping over it can
/// select another case than the source declares.
pub(crate) fn selected_case(
    ast: &Ast,
    id: Id<TypeNode>,
    selector: i128,
) -> Result<Option<(&MemberNode, Id<TypeNode>)>, EncodeError> {
    let mut fallback = None;
    for case in ast.cases_of(id) {
        let Ok(case_node) = ast.case(*case) else {
            return Err(EncodeError::NoWriter {
                member: crate::adapter::cst::handle_of(*case),
                kind: "a case the model does not hold",
            });
        };
        let Ok(member) = ast.member(case_node.member) else {
            return Err(EncodeError::NoWriter {
                member: crate::adapter::cst::handle_of(case_node.member),
                kind: "a member the model does not hold",
            });
        };
        let Some(member_type) = member.type_.resolved() else {
            return Err(EncodeError::NoWriter {
                member: member.name.clone(),
                kind: "union case with an unresolved type",
            });
        };
        for label in &case_node.labels {
            match label {
                // A `default:` case is selected by every value no other
                // label names.
                CaseLabel::Default => fallback = Some((member, member_type)),
                CaseLabel::Value(expr) => {
                    if ast
                        .eval_integral(*expr)
                        .is_ok_and(|value| value == selector)
                    {
                        return Ok(Some((member, member_type)));
                    }
                }
            }
        }
    }
    Ok(fallback)
}
