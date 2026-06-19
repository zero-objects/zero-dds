// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CosNaming-Exceptions — Spec §2.5.4.

use crate::name::Name;

/// `NotFoundReason` — Spec §2.5.4.6.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotFoundReason {
    /// `missing_node` — the path breaks off at a non-existent node.
    MissingNode,
    /// `not_context` — the referenced node is not a context.
    NotContext,
    /// `not_object` — the referenced node is a context, but not the
    /// object being looked up.
    NotObject,
}

/// Naming service error. Spec §2.5.4 lists the five normative user
/// exceptions; we aggregate them here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NamingError {
    /// `NotFound { why, rest_of_name }`.
    NotFound {
        /// Reason code.
        why: NotFoundReason,
        /// Remaining portion of the name from the failure point.
        rest_of_name: Name,
    },
    /// `CannotProceed { cxt, rest_of_name }` — the context could not
    /// perform the resolve step (e.g. federation problems). The `cxt`
    /// context is the caller's responsibility; we carry only the
    /// remaining name.
    CannotProceed {
        /// Remaining name.
        rest_of_name: Name,
    },
    /// `InvalidName` — name is empty or contains invalid entries.
    InvalidName,
    /// `AlreadyBound` — a binding with an identical name exists.
    AlreadyBound,
    /// `NotEmpty` — the context cannot be destroyed because it still
    /// contains bindings.
    NotEmpty,
}
