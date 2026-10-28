// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Model error type. Spec §10.

use crate::invariant::InvariantViolation;
use crate::kind::NodeKind;
use alloc::string::String;
use core::fmt;
use zerodds_ast_core::{AstCoreError, NodeId};

/// Failure of a model operation.
///
/// `#[non_exhaustive]` so that later phases can append variants —
/// invariant violations arrive with the edit surface — without that
/// being a breaking change.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AstError {
    /// Infrastructure failure.
    Core(AstCoreError),
    /// No node carries that identity.
    NotFound {
        /// The identity looked up.
        node: NodeId,
    },
    /// The node exists but is not the kind the operation required.
    WrongKind {
        /// The node.
        node: NodeId,
        /// Kind the operation required.
        expected: NodeKind,
        /// Kind actually found.
        found: NodeKind,
    },
    /// A name collided within its scope.
    DuplicateName {
        /// Scope the collision occurred in.
        scope: NodeId,
        /// The colliding name.
        name: String,
    },
    /// A commit broke one or more structural invariants and was rolled
    /// back. The model is byte-identical to its state before the
    /// transaction began. Spec §8.1.
    Invariant(alloc::vec::Vec<InvariantViolation>),
    /// A part was added to a type that does not hold that kind of part —
    /// an enumerator on a struct, a member on an enum.
    NotAnAggregate {
        /// The target type.
        node: NodeId,
        /// What kind of type it actually is.
        found: &'static str,
    },
    /// A type that already has a body was asked to receive one again.
    /// Two definitions of one name are a duplicate, not a completion.
    AlreadyComplete {
        /// The type that already has a body.
        node: NodeId,
    },
}

impl From<AstCoreError> for AstError {
    fn from(error: AstCoreError) -> Self {
        Self::Core(error)
    }
}

impl fmt::Display for AstError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Core(error) => write!(f, "{error}"),
            Self::NotFound { node } => write!(f, "no node with identity {node:?}"),
            Self::WrongKind {
                node,
                expected,
                found,
            } => write!(
                f,
                "node {node:?} is a {}, expected a {}",
                found.as_str(),
                expected.as_str()
            ),
            Self::DuplicateName { scope, name } => {
                write!(f, "name `{name}` already exists in scope {scope:?}")
            }
            Self::NotAnAggregate { node, found } => {
                write!(f, "node {node:?} is a {found} and holds no such part")
            }
            Self::AlreadyComplete { node } => {
                write!(f, "node {node:?} already has a body")
            }
            Self::Invariant(violations) => {
                write!(f, "commit rejected, {} violation(s):", violations.len())?;
                for violation in violations {
                    write!(f, " {violation};")?;
                }
                Ok(())
            }
        }
    }
}

impl core::error::Error for AstError {}
