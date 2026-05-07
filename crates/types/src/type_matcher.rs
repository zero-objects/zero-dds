// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Writer↔Reader Type-Matching (XTypes §7.2.4 + §7.6.3.7).
//!
//! Verbindet [`assignability::is_assignable`] mit der QoS-Policy
//! [`TypeConsistencyEnforcement`]: je nach TCE-Flags werden einzelne
//! Assignability-Rules abgeschwaecht oder verschaerft.
//!
//! Beispiel:
//!
//! ```
//! use zerodds_types::qos::TypeConsistencyEnforcement;
//! use zerodds_types::resolve::TypeRegistry;
//! use zerodds_types::type_matcher::TypeMatcher;
//! use zerodds_types::{PrimitiveKind, TypeIdentifier};
//!
//! let reg = TypeRegistry::new();
//! let tce = TypeConsistencyEnforcement::default();
//! let m = TypeMatcher::new(&tce);
//! let writer = TypeIdentifier::Primitive(PrimitiveKind::Int32);
//! let reader = TypeIdentifier::Primitive(PrimitiveKind::Int32);
//! assert!(m.match_types(&writer, &reader, &reg).is_match());
//! ```

use crate::assignability::{AssignabilityConfig, Assignable, is_assignable};
use crate::qos::{TypeConsistencyEnforcement, TypeConsistencyKind};
use crate::resolve::TypeRegistry;
use crate::type_identifier::TypeIdentifier;

/// Ergebnis eines Type-Matches. Identisch in Semantik zu [`Assignable`],
/// aber ein eigenstaendiger Typ fuer die Matcher-API (so wird der
/// Call-Site nicht an das interne `Assignable` gekoppelt).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeMatchResult {
    /// Writer-Type und Reader-Type sind fuer ein Match kompatibel.
    Matches,
    /// Inkompatibel — statische Begruendung.
    Incompatible {
        /// Kurze, statische Begruendung.
        reason: &'static str,
    },
}

impl TypeMatchResult {
    /// `true` wenn kompatibel.
    #[must_use]
    pub const fn is_match(&self) -> bool {
        matches!(self, Self::Matches)
    }

    fn from_assignable(a: Assignable) -> Self {
        match a {
            Assignable::Yes => Self::Matches,
            Assignable::No(reason) => Self::Incompatible { reason },
        }
    }
}

/// Facade um [`is_assignable`], die eine [`TypeConsistencyEnforcement`]-
/// Policy in die interne [`AssignabilityConfig`] uebersetzt.
///
/// Keine eigene State — hallt die TCE-Werte zum Call-Time durch.
#[derive(Debug, Clone, Copy)]
pub struct TypeMatcher<'a> {
    tce: &'a TypeConsistencyEnforcement,
}

impl<'a> TypeMatcher<'a> {
    /// Konstruktor mit einer TCE-Policy.
    #[must_use]
    pub const fn new(tce: &'a TypeConsistencyEnforcement) -> Self {
        Self { tce }
    }

    /// Prueft Writer↔Reader Type-Compatibility.
    ///
    /// `registry` stellt TypeObjects fuer `EquivalenceHash`-Referenzen
    /// bereit; ein leerer Registry passt fuer primitive/plain Typen.
    #[must_use]
    pub fn match_types(
        &self,
        writer: &TypeIdentifier,
        reader: &TypeIdentifier,
        registry: &TypeRegistry,
    ) -> TypeMatchResult {
        let cfg = self.build_config();
        TypeMatchResult::from_assignable(is_assignable(writer, reader, registry, &cfg))
    }

    /// Uebersetzt [`TypeConsistencyEnforcement`] nach
    /// [`AssignabilityConfig`].
    ///
    /// Mapping:
    /// - `kind == AllowTypeCoercion` ∧ ¬`prevent_type_widening`
    ///   → `allow_type_coercion = true`.
    /// - `force_type_validation` → `allow_type_coercion = false`
    ///   (uebertrumpft die vorige Regel, §7.6.3.7.1).
    /// - `max_depth` bleibt Default (kommt aus Resolver-Config).
    fn build_config(&self) -> AssignabilityConfig {
        let coerce = matches!(self.tce.kind, TypeConsistencyKind::AllowTypeCoercion)
            && !self.tce.prevent_type_widening;
        AssignabilityConfig {
            allow_type_coercion: if self.tce.force_type_validation {
                false
            } else {
                coerce
            },
            ignore_sequence_bounds: self.tce.ignore_sequence_bounds,
            ignore_string_bounds: self.tce.ignore_string_bounds,
            ignore_member_names: self.tce.ignore_member_names,
            ignore_literal_names: false,
            max_depth: crate::resolve::DEFAULT_MAX_RESOLVE_DEPTH,
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::field_reassign_with_default
)]
mod tests {
    use super::*;
    use crate::type_identifier::PrimitiveKind;

    fn reg() -> TypeRegistry {
        TypeRegistry::new()
    }

    #[test]
    fn identical_primitive_matches() {
        let tce = TypeConsistencyEnforcement::default();
        let m = TypeMatcher::new(&tce);
        let w = TypeIdentifier::Primitive(PrimitiveKind::Int32);
        assert_eq!(m.match_types(&w, &w, &reg()), TypeMatchResult::Matches);
    }

    #[test]
    fn widening_allowed_by_default_tce() {
        // TCE-Default: kind=AllowTypeCoercion, prevent_widening=false.
        let tce = TypeConsistencyEnforcement::default();
        let m = TypeMatcher::new(&tce);
        let w = TypeIdentifier::Primitive(PrimitiveKind::Int16);
        let r = TypeIdentifier::Primitive(PrimitiveKind::Int32);
        assert!(m.match_types(&w, &r, &reg()).is_match());
    }

    #[test]
    fn widening_blocked_by_prevent_type_widening() {
        let mut tce = TypeConsistencyEnforcement::default();
        tce.prevent_type_widening = true;
        let m = TypeMatcher::new(&tce);
        let w = TypeIdentifier::Primitive(PrimitiveKind::Int16);
        let r = TypeIdentifier::Primitive(PrimitiveKind::Int32);
        assert!(!m.match_types(&w, &r, &reg()).is_match());
    }

    #[test]
    fn force_type_validation_blocks_coercion() {
        let mut tce = TypeConsistencyEnforcement::default();
        tce.force_type_validation = true;
        let m = TypeMatcher::new(&tce);
        let w = TypeIdentifier::Primitive(PrimitiveKind::Int16);
        let r = TypeIdentifier::Primitive(PrimitiveKind::Int32);
        assert!(!m.match_types(&w, &r, &reg()).is_match());
    }

    #[test]
    fn disallow_type_coercion_blocks_widening() {
        let mut tce = TypeConsistencyEnforcement::default();
        tce.kind = TypeConsistencyKind::DisallowTypeCoercion;
        let m = TypeMatcher::new(&tce);
        let w = TypeIdentifier::Primitive(PrimitiveKind::Int16);
        let r = TypeIdentifier::Primitive(PrimitiveKind::Int32);
        assert!(!m.match_types(&w, &r, &reg()).is_match());
    }

    #[test]
    fn incompatible_reports_reason() {
        let tce = TypeConsistencyEnforcement::default();
        let m = TypeMatcher::new(&tce);
        let w = TypeIdentifier::Primitive(PrimitiveKind::Int64);
        let r = TypeIdentifier::Primitive(PrimitiveKind::Int16);
        match m.match_types(&w, &r, &reg()) {
            TypeMatchResult::Incompatible { reason } => {
                assert!(!reason.is_empty(), "reason must be non-empty");
            }
            TypeMatchResult::Matches => {
                panic!("narrowing i64→i16 must not match");
            }
        }
    }
}
