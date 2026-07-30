// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Writer↔Reader Type-Matching (XTypes §7.2.4 + §7.6.3.7).
//!
//! Connects [`assignability::is_assignable`] with the QoS policy
//! [`TypeConsistencyEnforcement`]: depending on the TCE flags, individual
//! assignability rules are relaxed or tightened.
//!
//! Example:
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

/// Result of a type match. Identical in semantics to [`Assignable`],
/// but a standalone type for the matcher API (so the
/// call site is not coupled to the internal `Assignable`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeMatchResult {
    /// Writer type and reader type are compatible for a match.
    Matches,
    /// Incompatible — static reason.
    Incompatible {
        /// Short, static reason.
        reason: &'static str,
    },
}

impl TypeMatchResult {
    /// `true` if compatible.
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

/// Facade over [`is_assignable`] that translates a [`TypeConsistencyEnforcement`]
/// policy into the internal [`AssignabilityConfig`].
///
/// No own state — passes the TCE values through at call time.
#[derive(Debug, Clone, Copy)]
pub struct TypeMatcher<'a> {
    tce: &'a TypeConsistencyEnforcement,
}

impl<'a> TypeMatcher<'a> {
    /// Constructor with a TCE policy.
    #[must_use]
    pub const fn new(tce: &'a TypeConsistencyEnforcement) -> Self {
        Self { tce }
    }

    /// Checks writer↔reader type compatibility.
    ///
    /// `registry` provides TypeObjects for `EquivalenceHash` references;
    /// an empty registry fits primitive/plain types.
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

    /// Translates [`TypeConsistencyEnforcement`] into
    /// [`AssignabilityConfig`].
    ///
    /// Mapping:
    /// - `kind == AllowTypeCoercion` ∧ ¬`prevent_type_widening`
    ///   → `allow_type_coercion = true`.
    /// - `force_type_validation` → `allow_type_coercion = false`
    ///   (overrides the previous rule, §7.6.3.7.1).
    /// - `max_depth` stays at the default (comes from the resolver config).
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
    clippy::expect_used,
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

    // ================================================================
    // Gap 2 (#24): structural, hash-referenced matching THROUGH a
    // populated TypeRegistry. These prove the registry is load-bearing:
    // the same hash-referenced pair resolves to a real structural verdict
    // only when the TypeObjects are present (an EMPTY registry can only
    // fall back to "unknown type objects for hash comparison" = No).
    // ================================================================

    use crate::builder::{Extensibility, TypeObjectBuilder};
    use crate::hash::compute_hash;
    use crate::type_object::{MinimalTypeObject, TypeObject};

    /// Registers a `MinimalStructType` under its `EquivalenceHash` and
    /// returns the endpoint `TypeIdentifier` (EquivalenceHashMinimal).
    fn register(
        reg: &mut TypeRegistry,
        st: crate::type_object::minimal::MinimalStructType,
    ) -> TypeIdentifier {
        let mto = MinimalTypeObject::Struct(st);
        let hash = compute_hash(&TypeObject::Minimal(mto.clone())).expect("hash");
        reg.insert_minimal(hash, mto);
        TypeIdentifier::EquivalenceHashMinimal(hash)
    }

    /// Appendable struct evolution: the WRITER added a trailing @optional
    /// member (writer ⊇ reader prefix, XTypes §7.2.4.4.4.4). Default TCE
    /// (AllowTypeCoercion) → the differing hashes resolve structurally to a
    /// MATCH once both TypeObjects are in the registry.
    #[test]
    fn appendable_trailing_optional_member_matches_via_registry() {
        let mut reg = TypeRegistry::new();
        // Reader V1: {a: i32, b: i32}, @appendable.
        let reader = TypeObjectBuilder::struct_type("::evo::V1")
            .extensibility(Extensibility::Appendable)
            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
            .member("b", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
            .build_minimal();
        // Writer V2 = V1 + trailing @optional c: i32, @appendable.
        let writer = TypeObjectBuilder::struct_type("::evo::V2")
            .extensibility(Extensibility::Appendable)
            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
            .member("b", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
            .member("c", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.optional()
            })
            .build_minimal();
        let w_id = register(&mut reg, writer);
        let r_id = register(&mut reg, reader);
        assert_ne!(w_id, r_id, "the two evolutions must have distinct hashes");

        let tce = TypeConsistencyEnforcement::default();
        let m = TypeMatcher::new(&tce);
        assert!(
            m.match_types(&w_id, &r_id, &reg).is_match(),
            "appendable writer-superset must match through the populated registry"
        );
        // Load-bearing proof: with an EMPTY registry the exact same call can
        // NOT resolve the differing hashes → no structural match.
        assert!(
            !m.match_types(&w_id, &r_id, &TypeRegistry::new()).is_match(),
            "empty registry must NOT resolve the differing hashes (old behavior)"
        );
    }

    /// A widened member (writer i16 → reader i32) matches under the default
    /// coercing TCE, but `force_type_validation` disables coercion
    /// (§7.6.3.7.1) → the structural check reports Incompatible. Both
    /// verdicts go through the populated registry.
    #[test]
    fn widened_member_incompatible_under_force_validation_via_registry() {
        let mut reg = TypeRegistry::new();
        // Writer: {a: i16}, @final (exact structural check, no appendable slack).
        let writer = TypeObjectBuilder::struct_type("::w::Widen")
            .extensibility(Extensibility::Final)
            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int16), |m| m)
            .build_minimal();
        // Reader: {a: i32}, @final.
        let reader = TypeObjectBuilder::struct_type("::w::Widen")
            .extensibility(Extensibility::Final)
            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
            .build_minimal();
        let w_id = register(&mut reg, writer);
        let r_id = register(&mut reg, reader);

        // Default TCE coerces the widening i16 → i32 → Matches.
        let coerce = TypeConsistencyEnforcement::default();
        assert!(
            TypeMatcher::new(&coerce)
                .match_types(&w_id, &r_id, &reg)
                .is_match(),
            "default TCE must coerce the i16→i32 widening"
        );

        // force_type_validation disables coercion → Incompatible.
        let mut strict = TypeConsistencyEnforcement::default();
        strict.force_type_validation = true;
        assert!(
            !TypeMatcher::new(&strict)
                .match_types(&w_id, &r_id, &reg)
                .is_match(),
            "force_type_validation must reject the widened member"
        );
    }
}
