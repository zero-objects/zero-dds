// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Writer↔Reader Type-Matching (XTypes §7.2.4 + §7.6.3.4).
//!
//! Connects [`assignability::is_assignable`](crate::assignability::is_assignable) with the QoS policy
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
    key_member_bounds: crate::assignability::KeyMemberBounds,
    strong_assignability: crate::assignability::StrongAssignability,
}

impl<'a> TypeMatcher<'a> {
    /// Constructor with a TCE policy.
    #[must_use]
    pub const fn new(tce: &'a TypeConsistencyEnforcement) -> Self {
        Self {
            tce,
            key_member_bounds: crate::assignability::KeyMemberBounds::Enforced,
            strong_assignability: crate::assignability::StrongAssignability::Equivalence,
        }
    }

    /// The Table 19 key-member axis this matcher decides under. Not a
    /// QoS: the policy is announced on the wire and the axis is not, so
    /// it is the runtime's setting and reaches both sides of a pair the
    /// same way.
    #[must_use]
    pub const fn with_key_member_bounds(
        mut self,
        key_member_bounds: crate::assignability::KeyMemberBounds,
    ) -> Self {
        self.key_member_bounds = key_member_bounds;
        self
    }

    /// The strong-assignability axis this matcher decides under, a
    /// setting of the runtime like [`Self::with_key_member_bounds`].
    #[must_use]
    pub const fn with_strong_assignability(
        mut self,
        strong_assignability: crate::assignability::StrongAssignability,
    ) -> Self {
        self.strong_assignability = strong_assignability;
        self
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
        // §7.6.3.4.2 Step 1: "If the Subscription does not allow type
        // coercion, then its type must be equivalent to the type of the
        // Publication." Equivalence is the relation of §7.3.4.7, not the
        // assignability a coercing subscription asks for.
        if !matches!(self.tce.kind, TypeConsistencyKind::AllowTypeCoercion) {
            return if crate::assignability::minimal_equivalent(writer, reader, registry, &cfg) {
                TypeMatchResult::Matches
            } else {
                TypeMatchResult::Incompatible {
                    reason: "DISALLOW_TYPE_COERCION: the types are not equivalent (XTypes 1.3 §7.6.3.4.2)",
                }
            };
        }
        TypeMatchResult::from_assignable(is_assignable(writer, reader, registry, &cfg))
    }

    /// Translates [`TypeConsistencyEnforcement`] into
    /// [`AssignabilityConfig`].
    ///
    /// Mapping (§7.6.3.4.1):
    /// - `kind == AllowTypeCoercion` → `allow_type_coercion = true`.
    /// - `prevent_type_widening`, `ignore_sequence_bounds`,
    ///   `ignore_string_bounds` and `ignore_member_names` "only apply when
    ///   the type consistency kind is ALLOW_TYPE_COERCION, otherwise the
    ///   fields are treated as though prevent_type_widening is set to true
    ///   and the others are set to false."
    /// - `force_type_validation` is not an assignability option: it makes
    ///   Step 2 of §7.6.3.4.2 fail when a side has no TypeObject, which the
    ///   caller decides before any identifier is compared.
    /// - `max_depth` stays at the default (comes from the resolver config).
    fn build_config(&self) -> AssignabilityConfig {
        let allow = matches!(self.tce.kind, TypeConsistencyKind::AllowTypeCoercion);
        AssignabilityConfig {
            allow_type_coercion: allow,
            ignore_sequence_bounds: allow && self.tce.ignore_sequence_bounds,
            ignore_string_bounds: allow && self.tce.ignore_string_bounds,
            ignore_member_names: allow && self.tce.ignore_member_names,
            ignore_literal_names: false,
            prevent_type_widening: !allow || self.tce.prevent_type_widening,
            max_depth: crate::resolve::DEFAULT_MAX_RESOLVE_DEPTH,
            key_member_bounds: self.key_member_bounds,
            appendable_is_delimited: true,
            strong_assignability: self.strong_assignability,
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

    /// XTypes 1.3 §7.2.4.4.4, Table 15, has no widening row, so the
    /// default TCE (kind=AllowTypeCoercion, prevent_widening=false) does
    /// not match int16 against int32 either.
    #[test]
    fn widening_rejected_by_default_tce() {
        let tce = TypeConsistencyEnforcement::default();
        let m = TypeMatcher::new(&tce);
        let w = TypeIdentifier::Primitive(PrimitiveKind::Int16);
        let r = TypeIdentifier::Primitive(PrimitiveKind::Int32);
        assert!(!m.match_types(&w, &r, &reg()).is_match());
    }

    /// The three ways a `TypeConsistencyEnforcement` can switch coercion
    /// off — `prevent_type_widening`, `force_type_validation`,
    /// `DisallowTypeCoercion` — each reject int16 against int32, and so
    /// does the coercing default: Table 15 leaves them nothing to block
    /// for a pair of primitives. The three flags keep their other duties
    /// (`force_type_validation` still fails Step 2 of §7.6.3.4.2 when a
    /// side announces no TypeObject), only the primitive verdict is no
    /// longer theirs to change.
    #[test]
    fn every_non_coercing_tce_agrees_with_the_default_on_primitives() {
        let w = TypeIdentifier::Primitive(PrimitiveKind::Int16);
        let r = TypeIdentifier::Primitive(PrimitiveKind::Int32);

        let mut prevent_widening = TypeConsistencyEnforcement::default();
        prevent_widening.prevent_type_widening = true;
        let mut force_validation = TypeConsistencyEnforcement::default();
        force_validation.force_type_validation = true;
        let mut disallow = TypeConsistencyEnforcement::default();
        disallow.kind = TypeConsistencyKind::DisallowTypeCoercion;

        for tce in [
            TypeConsistencyEnforcement::default(),
            prevent_widening,
            force_validation,
            disallow,
        ] {
            assert!(
                !TypeMatcher::new(&tce)
                    .match_types(&w, &r, &reg())
                    .is_match(),
                "int16 → int32 must not match under {tce:?}"
            );
        }
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

    /// A widened member (writer i16 → reader i32) is Incompatible under
    /// the default coercing TCE as well as under `force_type_validation`:
    /// XTypes 1.3 §7.2.4.4.4, Table 15, holds a struct member to the
    /// identical primitive type, and the member check is the same one
    /// `primitive_compatible` answers for a bare primitive pair. Both
    /// verdicts go through the populated registry, so this is the
    /// structural path and not the identity short-circuit.
    #[test]
    fn widened_member_incompatible_via_registry_under_every_tce() {
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

        // The coercing default rejects the i16 → i32 member: Table 15 has
        // no widening row for a member type either.
        let coerce = TypeConsistencyEnforcement::default();
        assert!(
            !TypeMatcher::new(&coerce)
                .match_types(&w_id, &r_id, &reg)
                .is_match(),
            "default TCE must reject the i16→i32 widened member"
        );

        // force_type_validation reaches the same verdict.
        let mut strict = TypeConsistencyEnforcement::default();
        strict.force_type_validation = true;
        assert!(
            !TypeMatcher::new(&strict)
                .match_types(&w_id, &r_id, &reg)
                .is_match(),
            "force_type_validation must reject the widened member"
        );
    }

    /// §7.6.3.4.2 Step 1: a subscription that does not allow type
    /// coercion needs a type equivalent to the publication's. The pair of
    /// `appendable_trailing_optional_member_matches_via_registry` is
    /// assignable and not equivalent: it matches under ALLOW_TYPE_COERCION
    /// and not under DISALLOW_TYPE_COERCION, and a type matches itself
    /// under both.
    #[test]
    fn disallow_type_coercion_needs_equivalent_types() {
        let mut reg = TypeRegistry::new();
        let reader = TypeObjectBuilder::struct_type("::evo::V1")
            .extensibility(Extensibility::Appendable)
            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
            .build_minimal();
        let writer = TypeObjectBuilder::struct_type("::evo::V1")
            .extensibility(Extensibility::Appendable)
            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
            .member("c", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.optional()
            })
            .build_minimal();
        let w_id = register(&mut reg, writer);
        let r_id = register(&mut reg, reader);

        let allow = TypeConsistencyEnforcement::default();
        assert!(
            TypeMatcher::new(&allow)
                .match_types(&w_id, &r_id, &reg)
                .is_match()
        );

        let mut disallow = TypeConsistencyEnforcement::default();
        disallow.kind = TypeConsistencyKind::DisallowTypeCoercion;
        let matcher = TypeMatcher::new(&disallow);
        assert!(!matcher.match_types(&w_id, &r_id, &reg).is_match());
        assert!(matcher.match_types(&r_id, &r_id, &reg).is_match());
    }
    /// §7.6.3.4.1: the four flags "only apply when the type consistency
    /// kind is ALLOW_TYPE_COERCION, otherwise the fields are treated as
    /// though prevent_type_widening is set to true and the others are set to
    /// false". The flags are set the other way round than that reading, so
    /// a config that passed them on would show.
    #[test]
    fn the_flags_apply_only_under_allow_type_coercion() {
        let mut tce = TypeConsistencyEnforcement::default();
        tce.prevent_type_widening = false;
        tce.ignore_sequence_bounds = true;
        tce.ignore_string_bounds = true;
        tce.ignore_member_names = true;

        tce.kind = TypeConsistencyKind::DisallowTypeCoercion;
        let cfg = TypeMatcher::new(&tce).build_config();
        assert!(cfg.prevent_type_widening);
        assert!(!cfg.ignore_sequence_bounds);
        assert!(!cfg.ignore_string_bounds);
        assert!(!cfg.ignore_member_names);
        assert!(!cfg.allow_type_coercion);

        tce.kind = TypeConsistencyKind::AllowTypeCoercion;
        let cfg = TypeMatcher::new(&tce).build_config();
        assert!(!cfg.prevent_type_widening);
        assert!(cfg.ignore_sequence_bounds);
        assert!(cfg.ignore_string_bounds);
        assert!(cfg.ignore_member_names);
        assert!(cfg.allow_type_coercion);

        tce.prevent_type_widening = true;
        tce.ignore_sequence_bounds = false;
        tce.ignore_string_bounds = false;
        tce.ignore_member_names = false;
        let cfg = TypeMatcher::new(&tce).build_config();
        assert!(cfg.prevent_type_widening);
        assert!(!cfg.ignore_sequence_bounds);
        assert!(!cfg.ignore_string_bounds);
        assert!(!cfg.ignore_member_names);
    }

    /// §7.6.3.4.2 Step 1: "If the subscription allows type coercion and the
    /// ignore_member_names flag is true [...] assignability checking shall
    /// ignore the member names in both Subscription and Publication types.
    /// That is, only member IDs will impact assignability." The name `x1`
    /// sits at id 1 in the publication and at id 2 in the subscription
    /// (§7.2.4.4.4.5: one name at two ids is refused), `x5` at id 5 on both.
    #[test]
    fn ignore_member_names_lets_only_the_member_ids_decide() {
        let mut reg = TypeRegistry::new();
        let writer = TypeObjectBuilder::struct_type("::X")
            .extensibility(Extensibility::Mutable)
            .member("x1", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.id(1)
            })
            .member("x5", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.id(5)
            })
            .build_minimal();
        let reader = TypeObjectBuilder::struct_type("::X")
            .extensibility(Extensibility::Mutable)
            .member("x1", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.id(2)
            })
            .member("x5", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.id(5)
            })
            .build_minimal();
        let w_id = register(&mut reg, writer);
        let r_id = register(&mut reg, reader);

        let mut names_count = TypeConsistencyEnforcement::default();
        names_count.ignore_member_names = false;
        assert!(
            !TypeMatcher::new(&names_count)
                .match_types(&w_id, &r_id, &reg)
                .is_match(),
            "a name at two ids is refused while names count"
        );

        let mut names_ignored = TypeConsistencyEnforcement::default();
        names_ignored.ignore_member_names = true;
        assert!(
            TypeMatcher::new(&names_ignored)
                .match_types(&w_id, &r_id, &reg)
                .is_match(),
            "with ignore_member_names only the ids impact assignability"
        );

        // The flag belongs to ALLOW_TYPE_COERCION; DISALLOW needs
        // equivalent types and does not read it.
        names_ignored.kind = TypeConsistencyKind::DisallowTypeCoercion;
        assert!(
            !TypeMatcher::new(&names_ignored)
                .match_types(&w_id, &r_id, &reg)
                .is_match()
        );
    }
}
