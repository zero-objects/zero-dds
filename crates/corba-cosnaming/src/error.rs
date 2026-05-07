// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CosNaming-Exceptions — Spec §2.5.4.

use crate::name::Name;

/// `NotFoundReason` — Spec §2.5.4.6.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotFoundReason {
    /// `missing_node` — Pfad bricht an einem nicht-existenten Knoten ab.
    MissingNode,
    /// `not_context` — referenzierter Knoten ist kein Context.
    NotContext,
    /// `not_object` — referenzierter Knoten ist ein Context, aber
    /// nicht das gesuchte Object.
    NotObject,
}

/// Naming-Service-Fehler. Spec §2.5.4 listet die fuenf normativen
/// User-Exceptions; wir aggregieren sie hier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NamingError {
    /// `NotFound { why, rest_of_name }`.
    NotFound {
        /// Reason-Code.
        why: NotFoundReason,
        /// Verbleibender Name-Anteil ab dem Fehlpunkt.
        rest_of_name: Name,
    },
    /// `CannotProceed { cxt, rest_of_name }` — der Context konnte den
    /// Resolve-Schritt nicht durchfuehren (z.B. Federation-Probleme).
    /// `cxt`-Kontext ist Caller-Eigenschaft; wir tragen nur den
    /// Rest-Namen.
    CannotProceed {
        /// Verbleibender Name.
        rest_of_name: Name,
    },
    /// `InvalidName` — Name leer oder enthaelt ungueltige Eintraege.
    InvalidName,
    /// `AlreadyBound` — Binding mit identischem Namen existiert.
    AlreadyBound,
    /// `NotEmpty` — Context kann nicht zerstoert werden, weil noch
    /// Bindings drin sind.
    NotEmpty,
}
