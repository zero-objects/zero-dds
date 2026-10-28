// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CCM 4.0 §6.7.2 primary-key constraints and the §6.7.3 factory/finder
//! body mapping.
//!
//! Spec sources:
//! * §6.7.2 (p. 35-36) — primary-key type constraints:
//!   - the type MUST derive from `Components::PrimaryKeyBase`,
//!   - it MUST NOT have private state members,
//!   - it MUST NOT contain interface references.
//! * §6.7.3 (p. 36-37) — factory and finder operations are mapped onto
//!   the Explicit interface with `raises (CreateFailure, ...)` and
//!   `raises (FinderFailure, ...)` respectively.
//!
//! Both constraints the old syntax tree could not check are checkable
//! here: a state member's `private` keyword is in the CORBA facet, and
//! an interface reference is an `ObjectRef` type rather than a scoped
//! name that might be anything.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use zerodds_ast::{Ast, InterfaceNode, StateVisibility, TypeKind, TypeNode, ValuetypeKind};
use zerodds_ast_core::{Id, Ref};

use crate::transform::{HomeEquivalent, TransformError};

// ============================================================================
//  Spec §6.7.2 primary-key constraints.
// ============================================================================

/// A §6.7.2 constraint the primary-key type breaks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrimaryKeyError {
    /// The type is not a concrete valuetype.
    NotValueType(String),
    /// The type does not inherit from `Components::PrimaryKeyBase`.
    NotDerivedFromPrimaryKeyBase(String),
    /// The type has private state members.
    HasPrivateStateMembers(String),
    /// A state member is an object reference, which §6.7.2 forbids.
    HasInterfaceReference(String),
}

impl core::fmt::Display for PrimaryKeyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotValueType(name) => write!(f, "`{name}` is not a concrete valuetype"),
            Self::NotDerivedFromPrimaryKeyBase(name) => {
                write!(
                    f,
                    "`{name}` does not derive from Components::PrimaryKeyBase"
                )
            }
            Self::HasPrivateStateMembers(name) => {
                write!(f, "`{name}` has private state members")
            }
            Self::HasInterfaceReference(name) => {
                write!(f, "`{name}` has a state member that is an object reference")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for PrimaryKeyError {}

/// Spec §6.7.2 — whether `key` is a valid primary-key type.
///
/// # Errors
/// [`PrimaryKeyError`], naming the constraint that was broken.
pub fn validate_primary_key(ast: &Ast, key: Id<TypeNode>) -> Result<(), PrimaryKeyError> {
    let Ok(node) = ast.type_(key) else {
        return Err(PrimaryKeyError::NotValueType(String::new()));
    };
    let name = node.name.clone();
    let TypeKind::Valuetype {
        kind, bases, state, ..
    } = &node.kind
    else {
        return Err(PrimaryKeyError::NotValueType(name));
    };
    // Constraint 1: a concrete value type.
    if *kind == ValuetypeKind::Abstract {
        return Err(PrimaryKeyError::NotValueType(name));
    }

    // Constraint 2: derived from Components::PrimaryKeyBase.
    let derives = bases
        .iter()
        .any(|base| names(ast, base) == ["Components", "PrimaryKeyBase"]);
    if !derives {
        return Err(PrimaryKeyError::NotDerivedFromPrimaryKeyBase(name));
    }

    // Constraints 3 and 4: state-member introspection. The old check
    // rejected every scoped-name member because a syntax tree could not
    // tell an interface from a struct; the model can, so a `CKey` with a
    // nested struct member is accepted and one holding an object
    // reference is not.
    for member in state.clone() {
        let Ok(member) = ast.member(member) else {
            continue;
        };
        let private = member
            .facets
            .corba
            .as_ref()
            .is_some_and(|corba| corba.state_visibility == Some(StateVisibility::Private));
        if private {
            return Err(PrimaryKeyError::HasPrivateStateMembers(name));
        }
        if is_object_reference(ast, &member.type_) {
            return Err(PrimaryKeyError::HasInterfaceReference(name));
        }
    }
    Ok(())
}

/// Whether a type reference names an object reference, through however
/// many aliases.
fn is_object_reference(ast: &Ast, reference: &Ref<TypeNode>) -> bool {
    let mut current = reference.resolved();
    // Aliases are shallow in practice; the bound keeps a cycle in a
    // partially built model from spinning here.
    for _ in 0..64 {
        let Some(id) = current else {
            return false;
        };
        let Ok(node) = ast.type_(id) else {
            return false;
        };
        match &node.kind {
            TypeKind::ObjectRef { .. } => return true,
            TypeKind::Alias { aliased } => current = aliased.resolved(),
            _ => return false,
        }
    }
    false
}

/// The scoped-name segments behind a type reference, resolved or not.
fn names(ast: &Ast, reference: &Ref<TypeNode>) -> Vec<String> {
    match reference {
        Ref::Resolved(id) => ast
            .any_of_type(*id)
            .ok()
            .and_then(|any| ast.scoped_name_of(any))
            .map(|name| name.segments().to_vec())
            .unwrap_or_default(),
        Ref::Pending(zerodds_ast_core::PendingRef::ScopedName(name)) => name.segments().to_vec(),
        Ref::Pending(_) | Ref::Broken(_) => Vec::new(),
    }
}

// ============================================================================
//  Spec §6.7.3 factory and finder body mapping.
// ============================================================================

/// One factory or finder operation the caller declares. Spec §6.7.3.1 /
/// §6.7.3.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitOp {
    /// Operation name, for example `create_widget`.
    pub name: String,
    /// Parameters, in declaration order. Every one is `in`, which
    /// §6.7.3.1 requires and this mapping enforces.
    pub params: Vec<(String, Ref<TypeNode>)>,
    /// The `raises` clause as declared, extended per §6.7.3.1.2 with
    /// `CreateFailure` or `FinderFailure`.
    pub raises: Vec<Ref<zerodds_ast::ExceptionNode>>,
}

impl InitOp {
    /// A factory or finder with no parameters and no declared `raises`.
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            params: Vec::new(),
            raises: Vec::new(),
        }
    }
}

/// Spec §6.7.3 — adds factory and finder operations to a home's Explicit
/// interface.
///
/// §6.7.3.1 — factory: `<componentType> <name>(<params>) raises
/// (Components::CreateFailure, ...)`.
/// §6.7.3.2 — finder: `<componentType> <name>(<params>) raises
/// (Components::FinderFailure, ...)`.
///
/// The return type is the component the home manages, taken from the
/// keyless `create` or the keyed `find_by_primary_key` the Implicit
/// interface already carries, so the two agree by construction rather
/// than by the caller restating it.
///
/// # Errors
/// [`TransformError::Model`] when the model rejects an edit.
pub fn apply_factory_finder_body(
    ast: &mut Ast,
    home: &HomeEquivalent,
    factories: &[InitOp],
    finders: &[InitOp],
) -> Result<(), TransformError> {
    let component = managed_component(ast, home.implicit);
    for (ops, failure) in [(factories, "CreateFailure"), (finders, "FinderFailure")] {
        for op in ops {
            let id = ast.add_operation(home.explicit, &op.name, component.clone(), false)?;
            for (name, type_) in &op.params {
                let _ = ast.add_param(id, name, zerodds_ast::ParamDirection::In, type_.clone())?;
            }
            ast.add_raises(
                id,
                Ref::Pending(zerodds_ast_core::PendingRef::ScopedName(
                    zerodds_ast_core::ScopedName::absolute(alloc::vec![
                        "Components".to_string(),
                        failure.to_string(),
                    ]),
                )),
            )?;
            for exception in &op.raises {
                ast.add_raises(id, exception.clone())?;
            }
        }
    }
    Ok(())
}

/// The component type a home's Implicit interface returns from `create`.
fn managed_component(ast: &Ast, implicit: Id<InterfaceNode>) -> Option<Ref<TypeNode>> {
    let operations = ast.interface(implicit).ok()?.operations.clone();
    operations.into_iter().find_map(|id| {
        let node = ast.operation(id).ok()?;
        (node.name == "create").then(|| node.return_type.clone())?
    })
}
