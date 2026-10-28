// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The two shapes every rule in every catalogue has. Spec §2.4.
//!
//! Written once, so two constructs mapped the same way have rules that
//! are the same value but for their names. A difference between two
//! rules is then a difference in the source rather than in how someone
//! happened to write the emitter — which is the whole reason the rules
//! are data.

use seesaw_tgg::rules::format::{CorrDecl, NodeDecl, Role, RuleDecl, SideDecl};

/// A rule mapping one node to one node, with nothing else.
#[must_use]
pub fn one_to_one(name: &str, left: &str, right: &str, corr: &str) -> RuleDecl {
    RuleDecl {
        name: name.to_string(),
        rank: 0,
        documentation: None,
        left: side("l", left),
        right: side("r", right),
        // `establishes`, because this correspondence creates what it
        // names. A `references` one matches something an earlier rule
        // already established, which is what the part rule below uses
        // to find the owner.
        corrs: vec![CorrDecl {
            typ: corr.to_string(),
            left: "l".to_string(),
            right: "r".to_string(),
            role: Role::Establishes,
            bindings: Vec::new(),
        }],
        joins: Vec::new(),
    }
}

/// A rule mapping a part of an owner, with the owner as context.
///
/// `owner_corr` is the correspondence the owner's own rule established;
/// referencing it is what ties the part to the right one. The owner is
/// matched, never created, which is what lets the two rules compose
/// rather than one rule having to do both.
#[must_use]
pub fn part_of(name: &str, left: (&str, &str, &str), right: (&str, &str, &str)) -> RuleDecl {
    let (l_part, l_owner, owner_corr) = left;
    let (r_part, r_owner, part_corr) = right;
    RuleDecl {
        name: name.to_string(),
        rank: 1,
        documentation: None,
        left: SideDecl {
            anchor: "part".to_string(),
            nodes: vec![node("part", l_part), context_node("owner", l_owner)],
            links: vec![("owner".to_string(), "part".to_string())],
            same_value_links: Vec::new(),
        },
        right: SideDecl {
            anchor: "part".to_string(),
            nodes: vec![node("part", r_part), node("owner", r_owner)],
            links: vec![("owner".to_string(), "part".to_string())],
            same_value_links: Vec::new(),
        },
        corrs: vec![
            CorrDecl {
                typ: part_corr.to_string(),
                left: "part".to_string(),
                right: "part".to_string(),
                role: Role::Establishes,
                bindings: Vec::new(),
            },
            CorrDecl {
                typ: owner_corr.to_string(),
                left: "owner".to_string(),
                right: "owner".to_string(),
                role: Role::References,
                bindings: Vec::new(),
            },
        ],
        joins: Vec::new(),
    }
}

fn side(anchor: &str, typ: &str) -> SideDecl {
    SideDecl {
        anchor: anchor.to_string(),
        nodes: vec![node(anchor, typ)],
        links: Vec::new(),
        same_value_links: Vec::new(),
    }
}

fn node(name: &str, typ: &str) -> NodeDecl {
    NodeDecl {
        name: name.to_string(),
        typ: typ.to_string(),
        predicate: None,
        context: false,
        same_as: None,
        constant: None,
    }
}

/// A node that must exist and is never created.
fn context_node(name: &str, typ: &str) -> NodeDecl {
    NodeDecl {
        name: name.to_string(),
        typ: typ.to_string(),
        predicate: None,
        context: true,
        same_as: None,
        constant: None,
    }
}
