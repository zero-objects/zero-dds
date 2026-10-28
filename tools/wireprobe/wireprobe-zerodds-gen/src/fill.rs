// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The two questions every column's sample fill asks of a structure's
//! sample, answered once for all of them: which keys of the sample are
//! members at all, and which part of it belongs to the base when the
//! structure inherits.

use serde_json::{Map, Value};
use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::Id;

/// The base of a structure that inherits, and its part of the sample.
///
/// XTypes 1.3 §7.5.1.4.1.3: the base is the first member, `parent`, and it
/// takes the keys of its own members; the structure takes the others.
#[derive(Debug, Clone)]
pub struct Inherited {
    /// The base structure.
    pub base: Id<TypeNode>,
    /// The keys of `object` that name members of the base, with their values.
    pub part: Value,
    own: Vec<String>,
}

impl Inherited {
    /// The base of `type_` and its part of `object`, or `None` when
    /// `type_` inherits nothing.
    #[must_use]
    pub fn of(ast: &Ast, type_: Id<TypeNode>, object: &Map<String, Value>) -> Option<Self> {
        let (base, names) = zerodds_lang_rules::emit::inherited_member_names(ast, type_)?;
        let part: Map<String, Value> = object
            .iter()
            .filter(|(name, _)| names.contains(name))
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect();
        let own = ast
            .members_of(type_)
            .iter()
            .filter_map(|id| ast.member(*id).ok())
            .map(|member| member.name.clone())
            .collect();
        Some(Self {
            base,
            part: Value::Object(part),
            own,
        })
    }
}

/// Whether the structure writes member `name` itself: every member when it
/// has no base, only its own when it has one (the base writes the rest).
#[must_use]
pub fn writes_member(inherited: Option<&Inherited>, name: &str) -> bool {
    inherited.is_none_or(|inherited| inherited.own.iter().any(|own| own == name))
}

/// Refuses a sample that names a key the structure has no member for.
///
/// # Errors
/// `"{path}.{name}: not a member; {declared:?}"` for the first such key.
pub fn refuse_undeclared(
    object: &Map<String, Value>,
    declared: &[&str],
    path: &str,
) -> Result<(), String> {
    for name in object.keys() {
        if !declared.contains(&name.as_str()) {
            return Err(format!("{path}.{name}: not a member; {declared:?}"));
        }
    }
    Ok(())
}
