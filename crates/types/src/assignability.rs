// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Type-Assignability (XTypes 1.3 §7.2.4.1).
//!
//! Determines whether a type `T1` can be assigned to a type `T2` —
//! corresponds to "compatible for publication/subscription match". The
//! rules depend on extensibility (final/appendable/mutable) +
//! TypeConsistencyEnforcement.
//!
//! Core rules for primitives, strings,
//! collections, aliases (via the resolver), enums + structs with
//! final/appendable/mutable semantics. Strict-vs-lax variant via
//! `AssignabilityConfig`.

use crate::resolve::{TypeRegistry, resolve_alias_chain};
use crate::type_identifier::{PrimitiveKind, TypeIdentifier};
use crate::type_object::flags::StructTypeFlag;
use crate::type_object::minimal::{MinimalStructType, MinimalTypeObject};

/// Error while flattening an inheritance chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InheritanceError {
    /// `base_type` points to an `EquivalenceHash` that is not in the
    /// registry.
    UnknownBase {
        /// The unresolved hash.
        hash: crate::type_identifier::EquivalenceHash,
    },
    /// `base_type` points to a TypeObject that is not a struct
    /// (e.g. an enum or an alias to an enum).
    BaseNotAStruct,
    /// Inheritance cycle detected.
    Cycle,
    /// Maximum depth exceeded.
    DepthExceeded {
        /// Limit.
        limit: usize,
    },
    /// Member name or member ID collides between base and derived
    /// (XTypes 1.3 §7.2.2.4.5).
    InheritanceConflict {
        /// Reference ID or name.
        member_id: u32,
        /// Description.
        reason: &'static str,
    },
}

/// Builds the "hypothetical flat type" structure from a single-inheritance
/// chain (XTypes 1.3 §7.2.2.4.5). Resolves `base_type` recursively and
/// concatenates the base members followed by the derived members. On member-name
/// or member-ID collisions, `InheritanceConflict` is returned.
///
/// `max_depth` limits the depth of the inheritance chain (cycle protection).
///
/// # Errors
/// See [`InheritanceError`].
pub fn flatten_inheritance(
    s: &MinimalStructType,
    registry: &TypeRegistry,
    max_depth: usize,
) -> Result<MinimalStructType, InheritanceError> {
    use alloc::collections::BTreeSet;

    let mut visited: BTreeSet<crate::type_identifier::EquivalenceHash> = BTreeSet::new();
    let mut chain: alloc::vec::Vec<MinimalStructType> = alloc::vec::Vec::new();
    let mut current = s.clone();
    for _ in 0..max_depth {
        let base_ti = current.header.base_type.clone();
        chain.push(current.clone());
        match base_ti {
            TypeIdentifier::None => break,
            TypeIdentifier::EquivalenceHashMinimal(h)
            | TypeIdentifier::EquivalenceHashComplete(h) => {
                if !visited.insert(h) {
                    return Err(InheritanceError::Cycle);
                }
                let to = match registry.get_minimal(&h) {
                    Some(MinimalTypeObject::Struct(b)) => b.clone(),
                    Some(_) => return Err(InheritanceError::BaseNotAStruct),
                    None => return Err(InheritanceError::UnknownBase { hash: h }),
                };
                current = to;
            }
            _ => return Err(InheritanceError::BaseNotAStruct),
        }
    }
    if chain
        .last()
        .is_none_or(|c| c.header.base_type != TypeIdentifier::None)
    {
        return Err(InheritanceError::DepthExceeded { limit: max_depth });
    }

    // chain is [Derived, Mid1, Mid2, ..., Root].
    // Spec §7.2.2.4.5: concatenate from root to derived (base members first).
    let mut flat_members: alloc::vec::Vec<crate::type_object::minimal::MinimalStructMember> =
        alloc::vec::Vec::new();
    let mut seen_ids: BTreeSet<u32> = BTreeSet::new();
    let mut seen_names: BTreeSet<crate::type_object::common::NameHash> = BTreeSet::new();
    for st in chain.iter().rev() {
        for m in &st.member_seq {
            if !seen_ids.insert(m.common.member_id) {
                return Err(InheritanceError::InheritanceConflict {
                    member_id: m.common.member_id,
                    reason: "member_id collides between base and derived",
                });
            }
            if !seen_names.insert(m.detail) {
                return Err(InheritanceError::InheritanceConflict {
                    member_id: m.common.member_id,
                    reason: "member name_hash collides between base and derived",
                });
            }
            flat_members.push(m.clone());
        }
    }

    // Result: the derived's header (incl. base_type=None after flatten) +
    // concatenated members.
    let Some(derived) = chain.first().cloned() else {
        return Err(InheritanceError::DepthExceeded { limit: max_depth });
    };
    let mut flat = derived;
    flat.header.base_type = TypeIdentifier::None;
    flat.member_seq = flat_members;
    Ok(flat)
}

/// Configuration for assignability checks.
///
/// The fields other than `max_depth` correspond 1:1 to the flags of the DDS-QoS
/// `TypeConsistencyEnforcement` (XTypes §7.6.3.4). `TypeMatcher`
/// translates a concrete TCE policy into this struct.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssignabilityConfig {
    /// Allow type coercion?
    ///
    /// Read by no check any more. It used to gate the primitive widening
    /// matrix in `primitive_compatible`, which XTypes 1.3 §7.2.4.4.4,
    /// Table 15, does not have — see that function. The field stays
    /// because `TypeMatcher` derives it from the
    /// `TypeConsistencyEnforcement` QoS (`kind`, `prevent_type_widening`,
    /// `force_type_validation`) and that mapping is what the QoS means;
    /// dropping it would delete the only place the three flags are read
    /// together.
    pub allow_type_coercion: bool,
    /// Ignore sequence bounds (the writer may be larger than the reader).
    pub ignore_sequence_bounds: bool,
    /// Ignore string bounds.
    pub ignore_string_bounds: bool,
    /// Ignore member names — mutable structs then match
    /// only via the `@id` member ID, not via the NameHash.
    pub ignore_member_names: bool,
    /// `@ignore_literal_names` globally (XTypes §7.2.4.4.7) — enum compat
    /// compares only ordinal values, not literal names. Additionally
    /// this can be set per `EnumTypeFlag::IGNORE_LITERAL_NAMES` on a
    /// single side; the disjunction wins.
    pub ignore_literal_names: bool,
    /// `prevent_type_widening` (XTypes 1.3 §7.6.3.4.1): the reader type
    /// may not be wider than the writer type — no member and no union
    /// label may exist on the reader that the writer does not declare.
    ///
    /// Assignability alone permits exactly that widening: an
    /// `@appendable` reader that has grown a member past the writer's
    /// last one reads it as its default, and a `@mutable` reader may
    /// declare ids the writer never sends. The flag is the reader
    /// saying it does not want the default, it wants the writer's
    /// member.
    pub prevent_type_widening: bool,
    /// Maximum depth for recursive resolution.
    pub max_depth: usize,
    /// Whether Table 19's three key-member bullets are enforced. See
    /// [`KeyMemberBounds`].
    pub key_member_bounds: KeyMemberBounds,
    /// Whether an `@appendable` type counts as a delimited type
    /// (XTypes 1.3 §7.2.4.2: "Types with extensibility kind APPENDABLE
    /// are delimited if serialized with encoding version 2 (DELIMITED_CDR)
    /// […] They are not delimited if serialized with encoding version
    /// 1."). Strong assignability (§7.2.4.3) asks whether the writer's
    /// type is delimited, so the answer depends on the data
    /// representation the pair uses: `true` (the default) for XCDR2,
    /// `false` for XCDR1.
    pub appendable_is_delimited: bool,
    /// Which equivalence strong assignability (§7.2.4.3) asks of a
    /// writer's type that is not delimited. See [`StrongAssignability`].
    pub strong_assignability: StrongAssignability,
}

impl Default for AssignabilityConfig {
    fn default() -> Self {
        Self {
            allow_type_coercion: false,
            ignore_sequence_bounds: true,
            ignore_string_bounds: true,
            ignore_member_names: false,
            ignore_literal_names: false,
            prevent_type_widening: false,
            max_depth: crate::resolve::DEFAULT_MAX_RESOLVE_DEPTH,
            key_member_bounds: KeyMemberBounds::Enforced,
            appendable_is_delimited: true,
            strong_assignability: StrongAssignability::Equivalence,
        }
    }
}

/// An axis over XTypes 1.3 §7.2.4.4.4.4, Table 19, the three key-member
/// bullets: a string, enumerated, sequence or map key member of the
/// reader must hold every key the writer can send — a bound at least
/// the writer's, every literal the writer names.
///
/// The clause states them without reference to the `ignore_*` policies
/// of §7.6.3.4.1, and the OMG `dds-xtypes` suite's four `struct_key_*`
/// cases that publish the wider key to the narrower reader expect a
/// match anyway (their own comments say they should not —
/// `docs/omg-xtypes-issues.md` §C1). Measured 2026-09-13 on the suite's
/// release binaries in self-pairs: RTI Connext 7.7.0, OpenDDS 3.35,
/// CoreDX 6.15.3 and HDDS 1.3.0 match all four; Cyclone 11.0.1 refuses
/// three of the four. So this is a disagreement between the text and
/// the field, and it gets what such a disagreement gets in this tree: an
/// axis whose default is the clause.
///
/// - [`Enforced`](Self::Enforced), the default: the bullets refuse the
///   pair. The note under Table 19 says why — two of the writer's
///   instances that agree on the reader's shorter key arrive as one.
/// - [`Ignored`](Self::Ignored): the bullets are not checked and the
///   ordinary member rule decides, as the four implementations above do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeyMemberBounds {
    /// Table 19's key-member bullets are enforced.
    #[default]
    Enforced,
    /// Table 19's key-member bullets are skipped.
    Ignored,
}

impl KeyMemberBounds {
    /// The spelling a configuration file or command line uses.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "enforced" => Some(Self::Enforced),
            "ignored" => Some(Self::Ignored),
            _ => None,
        }
    }

    /// The spelling [`Self::parse`] reads.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Enforced => "enforced",
            Self::Ignored => "ignored",
        }
    }
}

/// An axis over XTypes 1.3 §7.2.4.3, strong assignability: T1 is strongly
/// assignable from T2 when the two are equivalent under the MINIMAL
/// relation, or T1 is assignable from T2 and T2 is delimited. A FINAL
/// type is never delimited, and its MINIMAL TypeObject carries the
/// `name_hash` of every member, so two FINAL structs that differ only in
/// the names of their members are not equivalent, and an array or
/// sequence of one is not assignable from one of the other (Table 17).
///
/// The OMG `dds-xtypes` suite expects them to be:
/// `SFinal[10]_S[20]_SFinalAlt[10]_S[20]` and
/// `seq(SFinal,10)_seq(SFinalAlt,10)` pair an array or a sequence of
/// `struct { uint32 x1[20]; }` with one of `struct { uint32 altx1[20]; }`
/// and expect the sample ("the types are structurally equivalent"). A
/// disagreement between the text and the suite gets an axis whose
/// default is the clause.
///
/// - [`Equivalence`](Self::Equivalence), the default: the MINIMAL
///   relation of §7.3.4.7, member names included.
/// - [`NamesIgnored`](Self::NamesIgnored): a type that is not delimited
///   also counts when the two are assignable in both directions with
///   exact bounds and without member names, which for FINAL types means
///   the same members, by index and id, of the same types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StrongAssignability {
    /// MINIMAL equivalence as §7.3.4.7 defines it.
    #[default]
    Equivalence,
    /// Equivalence without the members' names.
    NamesIgnored,
}

impl StrongAssignability {
    /// The spelling a configuration file or command line uses.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "equivalence" => Some(Self::Equivalence),
            "names_ignored" => Some(Self::NamesIgnored),
            _ => None,
        }
    }

    /// The spelling [`Self::parse`] reads.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Equivalence => "equivalence",
            Self::NamesIgnored => "names_ignored",
        }
    }
}

/// Result of the assignability check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Assignable {
    /// Compatible.
    Yes,
    /// Not compatible, with a reason.
    No(&'static str),
}

impl Assignable {
    /// `true` if compatible.
    #[must_use]
    pub const fn is_yes(&self) -> bool {
        matches!(self, Self::Yes)
    }
}

/// Checks whether the writer type `w` is compatible for the reader type `r`.
///
/// zerodds-lint: recursion-depth 64
///
/// **Important on the depth number**: the `64` is the *default*
/// ([`DEFAULT_MAX_RESOLVE_DEPTH`](crate::resolve::DEFAULT_MAX_RESOLVE_DEPTH)) and **not** enforced by code —
/// `cfg.max_depth` is runtime-configurable. Tests run values up to
/// 512 (`resolve_depth_exceeded` tests); production callers should
/// use the default `AssignabilityConfig`, otherwise tune the value
/// explicitly with the risk assessment.
///
/// Recursion via `check_direct` → `is_assignable` (nested sequences,
/// struct members).
pub fn is_assignable(
    w: &TypeIdentifier,
    r: &TypeIdentifier,
    registry: &TypeRegistry,
    cfg: &AssignabilityConfig,
) -> Assignable {
    // Identity short-circuit FIRST (XTypes 1.3 §7.2.4.1: a type is always
    // assignable to itself). This must run *before* alias resolution, because
    // for a hash-referenced type (Minimal **or** Complete) that is absent from
    // the registry (e.g. a typed endpoint whose generated type carries a
    // complete TypeIdentifier and whose TypeObject was never registered),
    // `resolve_alias_chain` returns `Unknown` and would otherwise mask the
    // identity match. Two endpoints carrying the *same* complete TypeIdentifier
    // are by definition the same type.
    // Table 29's first row, before the identity shortcut: `TK_NONE |
    // N/A | Invalid identifier`. §7.2.4.1's "a type is always assignable
    // from itself" is a statement about *types*, and `TK_NONE` names
    // none — it is what `TypeInformation` writes for an equivalence form
    // the sender left out (§7.6.3.2.1). Answering `Yes` made two
    // endpoints that both carry the invalid identifier match each other,
    // which is a match on the absence of a type.
    if matches!(w, TypeIdentifier::None) || matches!(r, TypeIdentifier::None) {
        return Assignable::No(
            "TK_NONE is not a type identifier — Table 29 calls it an invalid identifier",
        );
    }

    if w == r {
        return Assignable::Yes;
    }

    // Alias resolution on both sides.
    let Ok(w) = resolve_alias_chain(w, registry, cfg.max_depth) else {
        return Assignable::No("writer alias resolution failed");
    };
    let Ok(r) = resolve_alias_chain(r, registry, cfg.max_depth) else {
        return Assignable::No("reader alias resolution failed");
    };

    check_direct(&w, &r, registry, cfg)
}

/// zerodds-lint: recursion-depth 64
///
/// Dispatches the assignability checks per TypeIdentifier variant.
/// Nested types (sequence, struct member) call `is_assignable` recursively
/// — depth cap via `cfg.max_depth`.
fn check_direct(
    w: &TypeIdentifier,
    r: &TypeIdentifier,
    registry: &TypeRegistry,
    cfg: &AssignabilityConfig,
) -> Assignable {
    // Exact identity → always Yes.
    if w == r {
        return Assignable::Yes;
    }

    match (w, r) {
        // Primitive → Primitive
        (TypeIdentifier::Primitive(wp), TypeIdentifier::Primitive(rp)) => {
            primitive_compatible(*wp, *rp)
        }
        // String compatibility. Small/Large is only an encoding detail;
        // bounds are filtered via `ignore_string_bounds`.
        (
            TypeIdentifier::String8Small { .. } | TypeIdentifier::String8Large { .. },
            TypeIdentifier::String8Small { .. } | TypeIdentifier::String8Large { .. },
        ) => {
            let (wb, rb) = (string_bound_u32_s8(w), string_bound_u32_s8(r));
            if !cfg.ignore_string_bounds && bound_exceeds(wb, rb) {
                Assignable::No("writer string8 bound exceeds reader bound")
            } else {
                Assignable::Yes
            }
        }
        (
            TypeIdentifier::String16Small { .. } | TypeIdentifier::String16Large { .. },
            TypeIdentifier::String16Small { .. } | TypeIdentifier::String16Large { .. },
        ) => {
            let (wb, rb) = (string_bound_u32_s16(w), string_bound_u32_s16(r));
            if !cfg.ignore_string_bounds && bound_exceeds(wb, rb) {
                Assignable::No("writer string16 bound exceeds reader bound")
            } else {
                Assignable::Yes
            }
        }

        // Sequence compatibility: Small ↔ Small/Large, Large ↔ Large/Small.
        // Small vs Large is only wire encoding; bounds are normalized to u32
        // and filtered per policy.
        (
            TypeIdentifier::PlainSequenceSmall { .. } | TypeIdentifier::PlainSequenceLarge { .. },
            TypeIdentifier::PlainSequenceSmall { .. } | TypeIdentifier::PlainSequenceLarge { .. },
        ) => {
            let (we, wb) = sequence_parts(w);
            let (re, rb) = sequence_parts(r);
            if !cfg.ignore_sequence_bounds && bound_exceeds(wb, rb) {
                return Assignable::No("writer sequence bound exceeds reader bound");
            }
            // Table 17: the element type is *strongly* assignable.
            strongly_assignable(we, re, registry, cfg)
        }

        // Array: fixed dimensions. The `bound_seq` comparison is structural;
        // ignore-bounds does not apply here (array bounds are not a policy matter).
        // Array: Small + Large among each other; normalize bounds to a u32 vec
        // and compare structurally.
        (
            TypeIdentifier::PlainArraySmall { .. } | TypeIdentifier::PlainArrayLarge { .. },
            TypeIdentifier::PlainArraySmall { .. } | TypeIdentifier::PlainArrayLarge { .. },
        ) => {
            let (we, wb) = array_parts(w);
            let (re, rb) = array_parts(r);
            if wb != rb {
                return Assignable::No("array bounds differ");
            }
            strongly_assignable(we, re, registry, cfg)
        }

        // Map: Small ↔ Small/Large, Large ↔ Large/Small.
        (
            TypeIdentifier::PlainMapSmall { .. } | TypeIdentifier::PlainMapLarge { .. },
            TypeIdentifier::PlainMapSmall { .. } | TypeIdentifier::PlainMapLarge { .. },
        ) => {
            let (we, wk, wb) = map_parts(w);
            let (re, rk, rb) = map_parts(r);
            if !cfg.ignore_sequence_bounds && bound_exceeds(wb, rb) {
                return Assignable::No("writer map bound exceeds reader bound");
            }
            match strongly_assignable(wk, rk, registry, cfg) {
                Assignable::Yes => strongly_assignable(we, re, registry, cfg),
                e => e,
            }
        }

        // Hash refs: both on Minimal → structural equality.
        (
            TypeIdentifier::EquivalenceHashMinimal(wh),
            TypeIdentifier::EquivalenceHashMinimal(rh),
        ) => {
            if wh == rh {
                return Assignable::Yes;
            }
            // Compare TypeObjects from the registry.
            match (registry.get_minimal(wh), registry.get_minimal(rh)) {
                (Some(wobj), Some(robj)) => check_minimal_types(wobj, robj, registry, cfg),
                _ => Assignable::No("unknown type objects for hash comparison"),
            }
        }
        // Hash refs: both on Complete (XTypes 1.3 §7.3.4.1 — a complete
        // TypeIdentifier carries the EquivalenceHashComplete). Equal hashes are
        // the same type. With both TypeObjects absent from the registry (the
        // typed-endpoint same-runtime case), equal hashes are the only thing we
        // can compare and are sufficient: identical complete hashes ⇒ identical
        // types. When the complete TypeObjects are registered we down-project to
        // the structural minimal comparison.
        (
            TypeIdentifier::EquivalenceHashComplete(wh),
            TypeIdentifier::EquivalenceHashComplete(rh),
        ) => {
            if wh == rh {
                return Assignable::Yes;
            }
            // Differing complete hashes: assignable if the registry holds both
            // types and their structures are compatible.
            //
            // The lookup is `get_complete`, not `get_minimal`. A
            // `CompleteTypeObject` is filed under the hash of the *complete*
            // object and its minimal projection under the hash of the
            // *minimal* one, and those two hashes are different by
            // construction (XTypes 1.3 §7.3.4.5 derives the two forms from
            // one model and hashes each). Asking `get_minimal` for a complete
            // hash therefore always missed — including for a type this very
            // participant had just registered — and every pair of
            // complete-hash endpoints with differing hashes fell out here as
            // "unknown", which the runtime reads as "not resolved yet" and
            // matches optimistically. That is the whole of the `dds-xtypes`
            // suite: 430 of its 670 entity expectations are pairs that must
            // NOT match, and every one of them matched.
            let (Some(wc), Some(rc)) = (registry.get_complete(wh), registry.get_complete(rh))
            else {
                return Assignable::No("unknown complete type objects for hash comparison");
            };
            let (Ok(wobj), Ok(robj)) = (wc.to_minimal(registry), rc.to_minimal(registry)) else {
                return Assignable::No("complete type objects do not project to minimal");
            };
            check_minimal_types(&wobj, &robj, registry, cfg)
        }
        // Hash refs: one side MINIMAL, the other COMPLETE. XTypes 1.3
        // §7.3.4.5 derives both forms from one type model, and §7.2.4
        // defines assignability over the minimal form alone — the complete
        // form only adds names and annotations, which the minimal form
        // reduces to a name hash. A mixed pair is therefore decided by
        // projecting the complete side down and running the same
        // structural comparison as the two same-form arms above.
        //
        // The pair is the cross-vendor default, not a corner case: a peer
        // announces the MINIMAL TypeIdentifier of its PID_TYPE_INFORMATION
        // while a local typed endpoint carries the COMPLETE one for its own
        // generated type. Without these two arms the pair fell through to
        // the `_ => "kinds do not match"` row at the end of this match, and
        // two structurally identical types were reported inconsistent — the
        // endpoints then never matched and no sample crossed.
        (
            TypeIdentifier::EquivalenceHashMinimal(wh),
            TypeIdentifier::EquivalenceHashComplete(rh),
        ) => {
            let (Some(wobj), Some(rc)) = (registry.get_minimal(wh), registry.get_complete(rh))
            else {
                return Assignable::No("unknown type objects for minimal/complete comparison");
            };
            let Ok(robj) = rc.to_minimal(registry) else {
                return Assignable::No("complete type object does not project to minimal");
            };
            check_minimal_types(wobj, &robj, registry, cfg)
        }
        (
            TypeIdentifier::EquivalenceHashComplete(wh),
            TypeIdentifier::EquivalenceHashMinimal(rh),
        ) => {
            let (Some(wc), Some(robj)) = (registry.get_complete(wh), registry.get_minimal(rh))
            else {
                return Assignable::No("unknown type objects for minimal/complete comparison");
            };
            let Ok(wobj) = wc.to_minimal(registry) else {
                return Assignable::No("complete type object does not project to minimal");
            };
            check_minimal_types(&wobj, robj, registry, cfg)
        }

        // Bitmask ↔ unsigned integer, the four remaining rows of Table
        // 15. A bitmask reaches this function as a hash-referenced
        // TypeObject and a primitive as `TypeIdentifier::Primitive`, so
        // the pair has to be named on both sides of the tuple; the row
        // itself is symmetric, and OpenDDS reads it the same way
        // (`dds/DCPS/XTypes/TypeAssignability.cpp`, `assignable_bitmask`
        // for bitmask-first and `assignable_primitive` for
        // primitive-first, one range table between them).
        (
            TypeIdentifier::EquivalenceHashMinimal(_) | TypeIdentifier::EquivalenceHashComplete(_),
            TypeIdentifier::Primitive(rp),
        ) => bitmask_uint_compatible(bitmask_bit_bound(w, registry), *rp),
        (
            TypeIdentifier::Primitive(wp),
            TypeIdentifier::EquivalenceHashMinimal(_) | TypeIdentifier::EquivalenceHashComplete(_),
        ) => bitmask_uint_compatible(bitmask_bit_bound(r, registry), *wp),

        // A type of a strongly connected component, against another one or
        // against a hashed type (§7.3.4.9). The component member is the
        // minimal object at its position, and the members of two recursive
        // types point back at each other, so a pair met again while it is
        // being decided is taken as the answer it is being given.
        (TypeIdentifier::StronglyConnectedComponent(_), _)
        | (_, TypeIdentifier::StronglyConnectedComponent(_)) => {
            let (Some(wobj), Some(robj)) =
                (resolve_minimal(w, registry), resolve_minimal(r, registry))
            else {
                return Assignable::No("unknown type objects for a strongly connected component");
            };
            registry
                .assuming(w, r, || check_minimal_types(&wobj, &robj, registry, cfg))
                .unwrap_or(Assignable::Yes)
        }

        _ => Assignable::No("kinds do not match"),
    }
}

/// The bit bound of a hash-referenced bitmask, where the identifier is
/// one and the registry holds it.
///
/// `None` covers three different things and the caller treats them
/// alike: the identifier is not a hash, the registry does not hold the
/// object, or the object is not a bitmask. None of the three is a
/// Table 15 row.
fn bitmask_bit_bound(ti: &TypeIdentifier, registry: &TypeRegistry) -> Option<u16> {
    let bound_of = |obj: &MinimalTypeObject| match obj {
        MinimalTypeObject::Bitmask(b) => Some(b.bit_bound),
        _ => None,
    };
    match ti {
        TypeIdentifier::EquivalenceHashMinimal(h) => bound_of(registry.get_minimal(h)?),
        TypeIdentifier::EquivalenceHashComplete(h) => {
            bound_of(&registry.get_complete(h)?.to_minimal(registry).ok()?)
        }
        _ => None,
    }
}

/// XTypes 1.3 §7.2.4.4.4, Table 15, rows two to five:
///
/// > | `UINT8_TYPE` | `BITMASK_TYPE` with `bound` in \[1, 8\] |
/// > | `UINT16_TYPE` | `BITMASK_TYPE` with `bound` in \[9, 16\] |
/// > | `UINT32_TYPE` | `BITMASK_TYPE` with `bound` in \[17, 32\] |
/// > | `UINT64_TYPE` | `BITMASK_TYPE` with `bound` in \[33, 64\] |
///
/// The ranges do not overlap and they do not nest: a bitmask of bound
/// 16 is a row with `uint16` and with nothing else, so `uint32` is not
/// assignable from it even though 16 bits fit in 32. That is the point
/// of the clause — the reader must not have to parse the sample
/// differently depending on which writer sent it — and it is what
/// OpenDDS implements (`TypeAssignability.cpp:1195` and `:1271`,
/// `1 <= b <= 8`, `9 <= b <= 16`, `17 <= b <= 32`, `33 <= b <= 64`).
fn bitmask_uint_compatible(bit_bound: Option<u16>, p: PrimitiveKind) -> Assignable {
    let Some(bit_bound) = bit_bound else {
        return Assignable::No("kinds do not match");
    };
    let holder = match bit_bound {
        1..=8 => PrimitiveKind::UInt8,
        9..=16 => PrimitiveKind::UInt16,
        17..=32 => PrimitiveKind::UInt32,
        33..=64 => PrimitiveKind::UInt64,
        _ => return Assignable::No("bitmask bit_bound is outside 1..=64"),
    };
    if holder == p {
        Assignable::Yes
    } else {
        Assignable::No("bitmask bit_bound does not match the integer width (XTypes 1.3 Table 15)")
    }
}

/// Whether a writer bound exceeds a reader bound, with `0` read as
/// "unbounded" on both sides.
///
/// XTypes 1.3 §7.3.4.5 writes the bound of an unbounded `string`,
/// `sequence` or `map` as `0` in its plain `TypeIdentifier`, and
/// §7.2.4.4.4.2's `STRING_TYPE`/`SEQUENCE_TYPE` rows compare *lengths*:
/// "T1.length >= T2.length" holds for a bounded T1 against an unbounded
/// T2 only if `0` is read as the largest length there is, not as the
/// smallest. Reading it arithmetically made `string` → `string<10>`
/// pass the check that `string<20>` → `string<10>` fails, which is the
/// suite's `string_string10_check` pair against `string20_string10_check`.
///
/// An unbounded *reader* accepts every writer bound, including another
/// unbounded one; that is the `r == 0` arm and it is checked first.
const fn bound_exceeds(writer: u32, reader: u32) -> bool {
    if reader == 0 {
        // The reader holds any length.
        return false;
    }
    // An unbounded writer can send more than any bound holds.
    writer == 0 || writer > reader
}

fn sequence_parts(ti: &TypeIdentifier) -> (&TypeIdentifier, u32) {
    match ti {
        TypeIdentifier::PlainSequenceSmall { element, bound, .. } => (element, u32::from(*bound)),
        TypeIdentifier::PlainSequenceLarge { element, bound, .. } => (element, *bound),
        _ => (ti, 0),
    }
}

fn array_parts(ti: &TypeIdentifier) -> (&TypeIdentifier, alloc::vec::Vec<u32>) {
    match ti {
        TypeIdentifier::PlainArraySmall {
            element,
            array_bounds,
            ..
        } => (
            element,
            array_bounds.iter().map(|b| u32::from(*b)).collect(),
        ),
        TypeIdentifier::PlainArrayLarge {
            element,
            array_bounds,
            ..
        } => (element, array_bounds.clone()),
        _ => (ti, alloc::vec::Vec::new()),
    }
}

fn map_parts(ti: &TypeIdentifier) -> (&TypeIdentifier, &TypeIdentifier, u32) {
    match ti {
        TypeIdentifier::PlainMapSmall {
            element,
            key,
            bound,
            ..
        } => (element, key, u32::from(*bound)),
        TypeIdentifier::PlainMapLarge {
            element,
            key,
            bound,
            ..
        } => (element, key, *bound),
        _ => (ti, ti, 0),
    }
}

fn string_bound_u32_s8(ti: &TypeIdentifier) -> u32 {
    match ti {
        TypeIdentifier::String8Small { bound } => u32::from(*bound),
        TypeIdentifier::String8Large { bound } => *bound,
        _ => 0,
    }
}

fn string_bound_u32_s16(ti: &TypeIdentifier) -> u32 {
    match ti {
        TypeIdentifier::String16Small { bound } => u32::from(*bound),
        TypeIdentifier::String16Large { bound } => *bound,
        _ => 0,
    }
}

/// XTypes 1.3 §7.2.4.4.4, Table 15 ("Type assignability of primitive
/// types"), taken literally. The table has exactly one row that a pair of
/// primitives can reach:
///
/// > | T1 Type Kind | T2 Type Kinds for which T1 is-assignable-from T2 is True |
/// > | Any Primitive Type | The same Primitive Type |
///
/// The table's four remaining rows are `UINT8`/`UINT16`/`UINT32`/`UINT64`
/// from `BITMASK_TYPE`. Those are cross-kind: a bitmask reaches
/// `check_direct` as a hash-referenced TypeObject, never as a
/// `PrimitiveKind`, so they cannot appear in this function. They live in
/// [`bitmask_uint_compatible`], which `check_direct` reaches from the two
/// hash-against-primitive arms.
///
/// There is no widening row, and the clause says why: "in order to
/// preserve high performance, they are designed to enable the
/// preservation of data representation, such that a DataReader is not
/// required to parse incoming samples differently based on the DataWriter
/// from which they originate."
///
/// This function used to carry a coercion matrix — widening yes,
/// narrowing no — gated on `AssignabilityConfig::allow_type_coercion`,
/// under which `uint16` was assignable to `int32` and `float32` to
/// `float64`. It was removed after measuring the reference
/// implementations against the OMG `dds-xtypes` suite (branch
/// `feature/xml-driver-enhanced`): its `xtypes_v2_primitive_test_suite`
/// is 169 of the 335 cases, 156 of them expecting `INCONSISTENT_TOPIC`,
/// and the suite's own `unsupported_test_manifest/` takes out no
/// primitive case for either RTI Connext 7.7 (38 exclusions, all of them
/// TryConstruct) or CoreDX 6.14 (no exclusions at all). Both pass all
/// 169; the coercion stood alone against them and against the clause.
///
/// `allow_type_coercion` is consequently no longer read here — Table 15
/// gives coercing and non-coercing enforcement the same verdict for
/// primitives, and the non-coercing verdict was always the stricter of
/// the two, so no pair gains a match by this.
fn primitive_compatible(w: PrimitiveKind, r: PrimitiveKind) -> Assignable {
    if w == r {
        Assignable::Yes
    } else {
        Assignable::No("primitive kinds differ (XTypes 1.3 Table 15)")
    }
}

/// Three-valued extensibility from the flag bits. Default (no flags) =
/// Appendable, per XTypes §7.2.2.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StructExt {
    Final,
    Appendable,
    Mutable,
}

fn struct_extensibility(flags: StructTypeFlag) -> StructExt {
    if flags.has(StructTypeFlag::IS_FINAL) {
        StructExt::Final
    } else if flags.has(StructTypeFlag::IS_MUTABLE) {
        StructExt::Mutable
    } else {
        StructExt::Appendable
    }
}

/// The `MinimalTypeObject` a hash identifier names, after the alias chain
/// is resolved, and `None` for anything that is not a hash or not known.
fn resolved_minimal_object(
    ti: &TypeIdentifier,
    registry: &TypeRegistry,
    cfg: &AssignabilityConfig,
) -> Option<MinimalTypeObject> {
    let resolved = resolve_alias_chain(ti, registry, cfg.max_depth).ok()?;
    resolve_minimal(&resolved, registry)
}

/// Whether `w` and `r` are equivalent under the MINIMAL relation
/// (XTypes 1.3 §7.3.4.7): "either they have equal Fully-Descriptive
/// TypeIdentifiers, or else they have equal Minimal TypeIdentifiers".
///
/// A COMPLETE hash is taken to the MINIMAL hash of the same type through
/// the registry before the two are compared; a type the registry cannot
/// project is not equivalent to anything but an identical identifier.
pub(crate) fn minimal_equivalent(
    w: &TypeIdentifier,
    r: &TypeIdentifier,
    registry: &TypeRegistry,
    cfg: &AssignabilityConfig,
) -> bool {
    if w == r {
        return true;
    }
    let (Ok(w), Ok(r)) = (
        resolve_alias_chain(w, registry, cfg.max_depth),
        resolve_alias_chain(r, registry, cfg.max_depth),
    ) else {
        return false;
    };
    if w == r {
        return true;
    }
    match (
        crate::type_object::to_minimal::minimal_identifier(&w, registry, 0),
        crate::type_object::to_minimal::minimal_identifier(&r, registry, 0),
    ) {
        (Ok(wm), Ok(rm)) => wm == rm,
        _ => false,
    }
}

/// Whether a type is a delimited type (XTypes 1.3 §7.2.4.2).
///
/// - Primitive types, enumerations, bitmasks, strings and wide strings
///   are delimited.
/// - A collection is delimited when its element type is, and for a map
///   when its key type is as well.
/// - Every other type of extensibility FINAL is not delimited; an
///   APPENDABLE one is delimited under encoding version 2
///   ([`AssignabilityConfig::appendable_is_delimited`]); a MUTABLE one is
///   delimited under both encoding versions.
///
/// A type the registry does not hold is not delimited: nothing is known
/// about how it is serialized.
///
/// zerodds-lint: recursion-depth 64 (collection nesting and aliases,
/// bounded by `cfg.max_depth`)
fn is_delimited(
    ti: &TypeIdentifier,
    registry: &TypeRegistry,
    cfg: &AssignabilityConfig,
    depth: usize,
) -> bool {
    if depth > cfg.max_depth {
        return false;
    }
    let Ok(ti) = resolve_alias_chain(ti, registry, cfg.max_depth) else {
        return false;
    };
    let ext_delimited = |ext: StructExt| match ext {
        StructExt::Final => false,
        StructExt::Appendable => cfg.appendable_is_delimited,
        StructExt::Mutable => true,
    };
    match &ti {
        TypeIdentifier::Primitive(_)
        | TypeIdentifier::String8Small { .. }
        | TypeIdentifier::String8Large { .. }
        | TypeIdentifier::String16Small { .. }
        | TypeIdentifier::String16Large { .. } => true,
        TypeIdentifier::PlainSequenceSmall { element, .. }
        | TypeIdentifier::PlainSequenceLarge { element, .. }
        | TypeIdentifier::PlainArraySmall { element, .. }
        | TypeIdentifier::PlainArrayLarge { element, .. } => {
            is_delimited(element, registry, cfg, depth + 1)
        }
        TypeIdentifier::PlainMapSmall { element, key, .. }
        | TypeIdentifier::PlainMapLarge { element, key, .. } => {
            is_delimited(key, registry, cfg, depth + 1)
                && is_delimited(element, registry, cfg, depth + 1)
        }
        TypeIdentifier::EquivalenceHashMinimal(_)
        | TypeIdentifier::EquivalenceHashComplete(_)
        | TypeIdentifier::StronglyConnectedComponent(_) => {
            match resolve_minimal(&ti, registry) {
                Some(MinimalTypeObject::Struct(s)) => {
                    ext_delimited(struct_extensibility(s.struct_flags))
                }
                Some(MinimalTypeObject::Union(u)) => {
                    ext_delimited(union_extensibility(u.union_flags))
                }
                Some(MinimalTypeObject::Enumerated(_) | MinimalTypeObject::Bitmask(_)) => true,
                Some(MinimalTypeObject::Sequence(c)) => {
                    is_delimited(&c.element.common.type_id, registry, cfg, depth + 1)
                }
                Some(MinimalTypeObject::Array(c)) => {
                    is_delimited(&c.element.common.type_id, registry, cfg, depth + 1)
                }
                Some(MinimalTypeObject::Map(c)) => {
                    is_delimited(&c.key.common.type_id, registry, cfg, depth + 1)
                        && is_delimited(&c.element.common.type_id, registry, cfg, depth + 1)
                }
                // A bitset is serialized in a fixed size and is not among
                // the types §7.2.4.2 names as delimited; an annotation
                // type is no data type.
                Some(
                    MinimalTypeObject::Bitset(_)
                    | MinimalTypeObject::Annotation(_)
                    | MinimalTypeObject::Alias(_)
                    | MinimalTypeObject::Extended { .. },
                )
                | None => false,
            }
        }
        _ => false,
    }
}

/// "T1 is strongly assignable from T2" (XTypes 1.3 §7.2.4.3): T1 and T2
/// are equivalent under the MINIMAL relation, or T1 is assignable from T2
/// and T2 is a delimited type. `w` is T2 and `r` is T1.
fn strongly_assignable(
    w: &TypeIdentifier,
    r: &TypeIdentifier,
    registry: &TypeRegistry,
    cfg: &AssignabilityConfig,
) -> Assignable {
    match is_assignable(w, r, registry, cfg) {
        Assignable::Yes => {}
        e => return e,
    }
    if minimal_equivalent(w, r, registry, cfg)
        || is_delimited(w, registry, cfg, 0)
        || (cfg.strong_assignability == StrongAssignability::NamesIgnored
            && equivalent_but_for_member_names(w, r, registry, cfg))
    {
        Assignable::Yes
    } else {
        Assignable::No(
            "not strongly assignable (§7.2.4.3): the types are not equivalent and the \
             writer's type is not delimited",
        )
    }
}

/// [`StrongAssignability::NamesIgnored`]: `w` and `r` are assignable in
/// both directions with exact sequence and string bounds, no widening and
/// member names left out.
fn equivalent_but_for_member_names(
    w: &TypeIdentifier,
    r: &TypeIdentifier,
    registry: &TypeRegistry,
    cfg: &AssignabilityConfig,
) -> bool {
    let exact = AssignabilityConfig {
        ignore_sequence_bounds: false,
        ignore_string_bounds: false,
        ignore_member_names: true,
        prevent_type_widening: true,
        ..*cfg
    };
    is_assignable(w, r, registry, &exact).is_yes() && is_assignable(r, w, registry, &exact).is_yes()
}

/// `KeyErased(T)` of an aggregated type (XTypes 1.3 §7.2.2.4.6): the type
/// without the key designation of its members, or of its discriminator.
/// Any other kind of type is its own `KeyErased`.
pub fn key_erased(object: MinimalTypeObject) -> MinimalTypeObject {
    use crate::type_object::flags::{StructMemberFlag, UnionDiscriminatorFlag};
    match object {
        MinimalTypeObject::Struct(mut s) => {
            for m in &mut s.member_seq {
                m.common.member_flags.0 &= !StructMemberFlag::IS_KEY;
            }
            MinimalTypeObject::Struct(s)
        }
        MinimalTypeObject::Union(mut u) => {
            u.discriminator.common.member_flags.0 &= !UnionDiscriminatorFlag::IS_KEY;
            MinimalTypeObject::Union(u)
        }
        other => other,
    }
}

/// Whether the type of a reader member is assignable from the type of the
/// writer member with the same id, under Table 19 (XTypes 1.3 §7.2.4.4.8):
/// `KeyErased(m1.type)` is-assignable-from `KeyErased(m2.type)`, and, for
/// members of the same index in an `@appendable` or `@final` type, the
/// member type is *strongly* assignable.
///
/// The key designation of a member's own type is not part of the
/// comparison: a struct or union that is the type of a member is compared
/// as the type `KeyErased` makes of it, which is the whole purpose of the
/// construct. The key bullets of Table 19 still apply to the members of
/// the types being compared at the top; they are checked by
/// [`key_members_fit`].
fn member_types_assignable(
    w: &TypeIdentifier,
    r: &TypeIdentifier,
    registry: &TypeRegistry,
    cfg: &AssignabilityConfig,
    strong: bool,
) -> Assignable {
    if w == r {
        return Assignable::Yes;
    }
    let aggregated = |ti: &TypeIdentifier| {
        matches!(
            resolved_minimal_object(ti, registry, cfg),
            Some(MinimalTypeObject::Struct(_) | MinimalTypeObject::Union(_))
        )
    };
    if !(aggregated(w) && aggregated(r)) {
        return if strong {
            strongly_assignable(w, r, registry, cfg)
        } else {
            is_assignable(w, r, registry, cfg)
        };
    }
    let (Some(wo), Some(ro)) = (
        resolved_minimal_object(w, registry, cfg),
        resolved_minimal_object(r, registry, cfg),
    ) else {
        return Assignable::No("a member type does not resolve");
    };
    match check_minimal_types(&key_erased(wo), &key_erased(ro), registry, cfg) {
        Assignable::Yes => {}
        e => return e,
    }
    if strong && !(minimal_equivalent(w, r, registry, cfg) || is_delimited(w, registry, cfg, 0)) {
        return Assignable::No(
            "not strongly assignable (§7.2.4.3): the types are not equivalent and the \
             writer's type is not delimited",
        );
    }
    Assignable::Yes
}

fn union_extensibility(flags: crate::type_object::flags::UnionTypeFlag) -> StructExt {
    if flags.0 & StructTypeFlag::IS_FINAL != 0 {
        StructExt::Final
    } else if flags.0 & StructTypeFlag::IS_MUTABLE != 0 {
        StructExt::Mutable
    } else {
        StructExt::Appendable
    }
}

fn check_minimal_types(
    w: &MinimalTypeObject,
    r: &MinimalTypeObject,
    registry: &TypeRegistry,
    cfg: &AssignabilityConfig,
) -> Assignable {
    match (w, r) {
        (MinimalTypeObject::Struct(ws), MinimalTypeObject::Struct(rs)) => {
            // Extensibility check: writer + reader must have
            // the same extensibility category (§7.2.4.4).
            // Bugfix (#10): consolidate the flag bits into a three-valued
            // enum comparison to correctly capture the corner case "both have
            // no flag bits set" = Appendable == Appendable.
            let w_ext = struct_extensibility(ws.struct_flags);
            let r_ext = struct_extensibility(rs.struct_flags);
            if w_ext != r_ext {
                return Assignable::No("extensibility mismatch");
            }
            // §7.2.2.4.5: a struct declared by inheritance is compared as
            // its "hypothetical flat type" — the base members first, then
            // its own — and never as the members it declares itself. The
            // three checks below read `member_seq` positionally (FINAL,
            // APPENDABLE) or by member id (MUTABLE), and an unflattened
            // derived struct enters them with its base's members missing
            // and its own ids shifted past them. `Shape5Default : Shape1
            // Default { float angle; }` then reaches §7.2.4.4.4.4 as a
            // one-member APPENDABLE struct whose only member sits at id 4,
            // and the shared prefix against `Shape1Default` (first member
            // at id 0) disagrees at position 0. Flattening here rather
            // than in each of the three keeps the clause in one place and
            // covers unions/aliases reached through member types, which
            // recurse back into this function.
            let w_flat;
            let r_flat;
            let (ws, rs) = if ws.header.base_type == TypeIdentifier::None
                && rs.header.base_type == TypeIdentifier::None
            {
                (ws, rs)
            } else {
                w_flat = match flatten_inheritance(ws, registry, cfg.max_depth) {
                    Ok(f) => f,
                    // A base type the registry cannot resolve is not
                    // "assignable by default": the flat type is unknown,
                    // so the comparison has no subject. Reject with the
                    // reason rather than compare the partial type — the
                    // caller (`DcpsRuntime`'s match path) probes for the
                    // missing hash separately and re-runs the check once
                    // the TypeLookup reply lands.
                    Err(_) => return Assignable::No("writer base type does not flatten"),
                };
                r_flat = match flatten_inheritance(rs, registry, cfg.max_depth) {
                    Ok(f) => f,
                    Err(_) => return Assignable::No("reader base type does not flatten"),
                };
                (&w_flat, &r_flat)
            };
            if !cfg.ignore_member_names {
                match struct_names_and_ids_agree(ws, rs) {
                    Assignable::Yes => {}
                    e => return e,
                }
            }
            match at_least_one_common_member_id(ws, rs) {
                Assignable::Yes => {}
                e => return e,
            }
            match key_members_fit(ws, rs, registry, cfg) {
                Assignable::Yes => {}
                e => return e,
            }
            if cfg.prevent_type_widening {
                match reader_is_not_wider(ws, rs) {
                    Assignable::Yes => {}
                    e => return e,
                }
            }
            match w_ext {
                StructExt::Final => check_final_structs(ws, rs, registry, cfg),
                StructExt::Appendable => check_appendable_structs(ws, rs, registry, cfg),
                StructExt::Mutable => check_mutable_structs(ws, rs, registry, cfg),
            }
        }
        (MinimalTypeObject::Enumerated(we), MinimalTypeObject::Enumerated(re)) => {
            check_enumerations(we, re, cfg)
        }
        (MinimalTypeObject::Union(wu), MinimalTypeObject::Union(ru)) => {
            check_unions(wu, ru, registry, cfg)
        }
        (MinimalTypeObject::Bitmask(wb), MinimalTypeObject::Bitmask(rb)) => {
            // Table 18, `BITMASK_TYPE`, verbatim and in full:
            //
            // > `BITMASK_TYPE` if and only if `T1.bound == T2.bound`
            //
            // and the object-construction column beside it: "Preserve
            // bit values by index **for all bits identified in both T1
            // and T2**."
            //
            // The bound and nothing else. This used to demand that every
            // flag the writer can set be a flag the reader knows, on the
            // enumeration rule's model — but the enumeration rule is
            // about *values*, where an unrecognised one has no meaning,
            // and a bitmask's bits are positions in a word of a fixed
            // width. The construction column rules out the reading it
            // was built on: "for all bits identified in both" says in so
            // many words that bits only one side names are expected, and
            // what happens to them.
            //
            // Two shapes it refused. Two bitmasks of `@bit_bound(8)`
            // whose flags sit at different positions — the reader reads
            // eight bits either way and ignores the ones it has no name
            // for. And a writer with one flag more than the reader,
            // which is what an evolved bitmask looks like from the older
            // side. `as-bm-same-bound` and `as-bm-writer-extra-flag` of
            // `docs/spec-coverage/dds-xtypes-1.3.gaps.md` §A.6 are those
            // two.
            if wb.bit_bound != rb.bit_bound {
                return Assignable::No("bitmask bit_bound mismatch");
            }
            Assignable::Yes
        }
        (MinimalTypeObject::Bitset(wb), MinimalTypeObject::Bitset(rb)) => {
            // §7.2.4.4.4.7. A bitfield's place is a bit position and its
            // width and holder decide how many bits are read there, so
            // all three have to agree; a reader missing a field the
            // writer sets would read that field's bits as part of
            // another.
            for wf in &wb.field_seq {
                let found = rb.field_seq.iter().any(|rf| {
                    rf.common.position == wf.common.position
                        && rf.common.bitcount == wf.common.bitcount
                        && rf.common.holder_type == wf.common.holder_type
                        && (cfg.ignore_member_names || rf.name_hash == wf.name_hash)
                });
                if !found {
                    return Assignable::No("bitset writer bitfield unknown in reader");
                }
            }
            Assignable::Yes
        }
        (MinimalTypeObject::Alias(wa), MinimalTypeObject::Alias(ra)) => {
            // §7.2.4.4.3: an alias is its target. `resolve_alias_chain`
            // takes both sides down to the target before this function
            // is reached whenever the alias is in the registry; this arm
            // is the case where a *member's* type is a registered alias
            // whose object arrived here directly.
            is_assignable(
                &wa.body.common.related_type,
                &ra.body.common.related_type,
                registry,
                cfg,
            )
        }
        (MinimalTypeObject::Sequence(ws), MinimalTypeObject::Sequence(rs)) => {
            // An anonymous collection reached by hash rather than by a
            // plain identifier. Same rule as the plain form in
            // `check_direct`: the bound is a policy matter, the element
            // type is not.
            if !cfg.ignore_sequence_bounds && bound_exceeds(ws.bound, rs.bound) {
                return Assignable::No("writer sequence bound exceeds reader bound");
            }
            strongly_assignable(
                &ws.element.common.type_id,
                &rs.element.common.type_id,
                registry,
                cfg,
            )
        }
        (MinimalTypeObject::Array(wa), MinimalTypeObject::Array(ra)) => {
            if wa.bound_seq != ra.bound_seq {
                return Assignable::No("array bounds differ");
            }
            strongly_assignable(
                &wa.element.common.type_id,
                &ra.element.common.type_id,
                registry,
                cfg,
            )
        }
        (MinimalTypeObject::Map(wm), MinimalTypeObject::Map(rm)) => {
            if !cfg.ignore_sequence_bounds && bound_exceeds(wm.bound, rm.bound) {
                return Assignable::No("writer map bound exceeds reader bound");
            }
            match strongly_assignable(
                &wm.key.common.type_id,
                &rm.key.common.type_id,
                registry,
                cfg,
            ) {
                Assignable::Yes => {}
                e => return e,
            }
            strongly_assignable(
                &wm.element.common.type_id,
                &rm.element.common.type_id,
                registry,
                cfg,
            )
        }
        (MinimalTypeObject::Annotation(_), MinimalTypeObject::Annotation(_)) => {
            // §7.2.4 is about the types a sample can carry, and an
            // annotation type carries none: it describes a declaration.
            // Saying "assignable" would claim a compatibility that has
            // no meaning, so the pair is refused with the reason named
            // rather than with the kind-mismatch message, which would be
            // untrue — the two kinds do match.
            Assignable::No("annotation types are not data types and §7.2.4 does not relate them")
        }
        _ => Assignable::No("type kinds do not match"),
    }
}

/// XTypes 1.3 §7.2.4.4.4.3 — enumerated types.
///
/// The rule is about *agreement*, not about containment. What it asks
/// of a non-`@final` pair is that the two enumerations do not
/// contradict each other:
///
/// > Any literals that have the same name also have the same value and
/// > any literals that have the same value also have the same name.
///
/// A literal that only one side declares breaks neither, so an
/// enumeration that has grown a literal is assignable to one that has
/// not. That is what makes the try-construct cases possible at all: the
/// suite's `tryc_enum_1` has a writer whose `E1` carries `VAL3`, a
/// reader whose `E2` does not, and it expects the two to **match** and
/// the sample to be handled by `@try_construct` on the way in
/// (§7.2.2.7). A reader that refused the match at discovery would never
/// reach the annotation.
///
/// This implementation used to require the writer's literals to be a
/// subset of the reader's — "the reader may have additional literals" —
/// which is the containment rule and not this one. It refused seven of
/// the suite's cases at discovery.
///
/// `@final` is the exception, and there containment is the rule twice
/// over: the two literal sets have to be the same size and every
/// literal has to be on both sides. OpenDDS
/// `dds/DCPS/XTypes/TypeAssignability.cpp::assignable_enum` splits the
/// two branches the same way.
fn check_enumerations(
    w: &crate::type_object::minimal::MinimalEnumeratedType,
    r: &crate::type_object::minimal::MinimalEnumeratedType,
    cfg: &AssignabilityConfig,
) -> Assignable {
    use crate::type_object::flags::EnumTypeFlag;
    use crate::type_object::flags::StructTypeFlag;

    // Table 18: "T1.extensibility == T2.extensibility". An enumeration
    // whose flag word names no extensibility kind has the one §7.3.1.2.1.8
    // gives a type that does not indicate it: "the type shall be
    // considered appendable". Fast DDS writes such a word (`enum_flags`
    // zero), RTI Connext, OpenDDS and Cyclone write the `TypeFlag`
    // position; a zero word compares as APPENDABLE and is not "left
    // alone", so a `@final` enumeration is not assignable from or to it.
    let ext_mask =
        StructTypeFlag::IS_FINAL | StructTypeFlag::IS_APPENDABLE | StructTypeFlag::IS_MUTABLE;
    let enum_extensibility = |flags: u16| {
        if flags & StructTypeFlag::IS_FINAL != 0 {
            StructExt::Final
        } else if flags & StructTypeFlag::IS_MUTABLE != 0 {
            StructExt::Mutable
        } else {
            StructExt::Appendable
        }
    };
    let w_ext = w.enum_flags.0 & ext_mask;
    let r_ext = r.enum_flags.0 & ext_mask;
    if enum_extensibility(w_ext) != enum_extensibility(r_ext) {
        return Assignable::No("enum extensibility mismatch");
    }

    // The bit bound is the wire width and there is no coercion of it —
    // DDSXTY14-34.
    if w.header.common.bit_bound != r.header.common.bit_bound {
        return Assignable::No("enum bit_bound mismatch");
    }

    // §7.3.1.2.1.11, verbatim: "If the @ignore_literal_names annotation
    // has the value TRUE in **both** enumerated types, then the literal
    // names are ignored for the purpose of checking the assignability of
    // the two Enumerated Types. Otherwise the literal names are
    // considered for type assignability checking."
    //
    // **Both**, not either. This read `||`, which let one side alone
    // decide that the other's literal names do not matter — a writer
    // could annotate its own enum and be assignable to a reader that had
    // said nothing, and the reader would store a literal under a name it
    // does not carry. The clause's own example annotates both types.
    //
    // `cfg.ignore_literal_names` stays a separate disjunct: it is this
    // process's own switch and not a claim about the peer's type.
    let ignore_names = cfg.ignore_literal_names
        || (w.enum_flags.has(EnumTypeFlag::IGNORE_LITERAL_NAMES)
            && r.enum_flags.has(EnumTypeFlag::IGNORE_LITERAL_NAMES));

    let is_final = (w_ext | r_ext) & StructTypeFlag::IS_FINAL != 0;
    if is_final {
        if w.literal_seq.len() != r.literal_seq.len() {
            return Assignable::No("final enum literal count mismatch");
        }
        for wl in &w.literal_seq {
            let found = r.literal_seq.iter().any(|rl| {
                rl.common.value == wl.common.value && (ignore_names || rl.detail == wl.detail)
            });
            if !found {
                return Assignable::No("final enum: a writer literal is not in the reader");
            }
        }
        return Assignable::Yes;
    }

    for wl in &w.literal_seq {
        for rl in &r.literal_seq {
            let same_value = wl.common.value == rl.common.value;
            if ignore_names {
                continue;
            }
            let same_name = wl.detail == rl.detail;
            if same_name != same_value {
                return Assignable::No(
                    "an enum literal name and its value do not agree across the two types",
                );
            }
        }
    }
    Assignable::Yes
}

/// Whether a member is one the other side may not be missing.
///
/// XTypes 1.3 §7.2.4.4.8, Table 19: "Members for which both optional is
/// false and must_understand is true in either T1 or T2 appear (i.e.,
/// have a corresponding member of the same member ID) in both T1 and T2."
/// A member that is `@optional` is therefore not one the peer may not be
/// missing, `@must_understand` or not: its absence from a sample is legal
/// and so is its absence from the type.
///
/// `@key` is folded in here because §7.2.2.4.4.4.8 says a key member's
/// must-understand property "shall always be true", so a key member is a
/// must-understand member by the clause even where the flag was not
/// written. `docs/omg-xtypes-issues.md` §B3 is the report that no
/// implementation writes it.
fn must_not_be_missing(flags: crate::type_object::flags::StructMemberFlag) -> bool {
    use crate::type_object::flags::StructMemberFlag;
    (flags.has(StructMemberFlag::IS_MUST_UNDERSTAND) && !flags.has(StructMemberFlag::IS_OPTIONAL))
        || flags.has(StructMemberFlag::IS_KEY)
}

/// One pair of members with the same id or the same position: names and
/// types.
///
/// `positional` is the `@appendable` and `@final` condition of Table 19:
/// the members of the same index have "the same setting for the
/// 'optional' attribute and the T1 member type is strongly assignable
/// from the T2 member type". Without it the member types are compared
/// as `KeyErased` types and no more.
fn check_member_pair(
    w: &crate::type_object::minimal::MinimalStructMember,
    r: &crate::type_object::minimal::MinimalStructMember,
    registry: &TypeRegistry,
    cfg: &AssignabilityConfig,
    name_mismatch: &'static str,
    positional: bool,
) -> Assignable {
    use crate::type_object::flags::StructMemberFlag;
    if !cfg.ignore_member_names && w.detail != r.detail {
        return Assignable::No(name_mismatch);
    }
    if positional
        && w.common.member_flags.has(StructMemberFlag::IS_OPTIONAL)
            != r.common.member_flags.has(StructMemberFlag::IS_OPTIONAL)
    {
        return Assignable::No(
            "Table 19: members of the same index differ in their `optional` attribute",
        );
    }
    member_types_assignable(
        &w.common.member_type_id,
        &r.common.member_type_id,
        registry,
        cfg,
        positional,
    )
}

/// §7.2.4.4.4.4, the rule that is neither positional nor by id:
///
/// > Any members in T1 and T2 that have the same name also have the same
/// > ID and any members with the same ID also have the same name.
///
/// It is a *cross* check over the two member sequences and not a
/// comparison of matched pairs, which is why it cannot live in
/// [`check_member_pair`]: the pair it refuses is a writer member and a
/// reader member that the id matching never brings together — the
/// suite's `struct_1`/`struct_2`, where `x1` sits at id 1 on one side
/// and at id 2 on the other and both sides also carry `x5` at id 5, so
/// the id walk finds a clean match and never looks at `x1`.
///
/// The `TypeConsistencyEnforcement` flag `ignore_member_names`
/// (§7.6.3.4.1) is exactly the switch that turns this rule off, and the
/// caller has already read it into `cfg`.
///
/// OpenDDS reads the clause the same way —
/// `dds/DCPS/XTypes/TypeAssignability.cpp`, `assignable_struct`: "Any
/// members in T1 and T2 that have the same name also have the same ID,
/// and vice versa", guarded by `if (!type_consistency_.ignore_member_names)`.
fn struct_names_and_ids_agree(
    w: &crate::type_object::minimal::MinimalStructType,
    r: &crate::type_object::minimal::MinimalStructType,
) -> Assignable {
    for wm in &w.member_seq {
        for rm in &r.member_seq {
            let same_name = wm.detail == rm.detail;
            let same_id = wm.common.member_id == rm.common.member_id;
            if same_name != same_id {
                return Assignable::No(
                    "a member name and a member id do not agree across the two types",
                );
            }
        }
    }
    Assignable::Yes
}

/// The same rule over union members (§7.2.4.4.4.5 repeats it for
/// unions; OpenDDS's `assignable_union` carries it as "Members with the
/// same ID must have the same name, and vice versa").
fn union_names_and_ids_agree(
    w: &crate::type_object::minimal::MinimalUnionType,
    r: &crate::type_object::minimal::MinimalUnionType,
) -> Assignable {
    for wm in &w.member_seq {
        for rm in &r.member_seq {
            let same_name = wm.detail == rm.detail;
            let same_id = wm.common.member_id == rm.common.member_id;
            if same_name != same_id {
                return Assignable::No(
                    "a union member name and member id do not agree across the two types",
                );
            }
        }
    }
    Assignable::Yes
}

/// §7.2.4.4.4.4: "There is at least one member m1 of T1 and one
/// corresponding member m2 of T2 such that m1.id == m2.id."
///
/// The rule that makes two `@mutable` structs with no id in common
/// incompatible instead of trivially compatible. Without it every pair
/// of mutable structs passed: the per-member walk only ever asks about
/// ids that *are* shared, and a reader whose ids the writer never sends
/// has nothing to disagree about. The suite's `struct_5`/`struct_6`,
/// one member each at id 1 and id 2.
///
/// The exception is the pair that declares no members at all. The
/// clause presupposes members; two empty structures are the same type
/// and §7.2.4.1 ("a type is always assignable from itself") answers
/// them, whereas the rule read literally would make an empty structure
/// assignable from nothing, itself included. An empty type against a
/// non-empty one still has no common id and is still refused.
fn at_least_one_common_member_id(
    w: &crate::type_object::minimal::MinimalStructType,
    r: &crate::type_object::minimal::MinimalStructType,
) -> Assignable {
    if w.member_seq.is_empty() && r.member_seq.is_empty() {
        return Assignable::Yes;
    }
    let shared = w.member_seq.iter().any(|wm| {
        r.member_seq
            .iter()
            .any(|rm| rm.common.member_id == wm.common.member_id)
    });
    if shared {
        Assignable::Yes
    } else {
        Assignable::No("the two structs have no member id in common")
    }
}

/// The `MinimalTypeObject` a hash identifier names, where the registry
/// holds it.
///
/// `None` covers three different things and every caller treats them
/// alike: the identifier is not a hash, the registry does not hold the
/// object, or a `COMPLETE` object does not reduce to a minimal one.
fn resolve_minimal(ti: &TypeIdentifier, registry: &TypeRegistry) -> Option<MinimalTypeObject> {
    match ti {
        TypeIdentifier::EquivalenceHashMinimal(h) => registry.get_minimal(h).cloned(),
        TypeIdentifier::EquivalenceHashComplete(h) => {
            registry.get_complete(h)?.to_minimal(registry).ok()
        }
        // A member of a strongly connected component, by its position in
        // the component (§7.3.4.9.2 step 4b).
        TypeIdentifier::StronglyConnectedComponent(scc) => {
            registry.component_minimal_member(scc).cloned()
        }
        _ => None,
    }
}

/// The declared length of a string, sequence or map identifier, and
/// `None` for anything that has none.
///
/// Zero is the specification's own "unbounded" (Table 29 on
/// `TI_STRING8_SMALL`: "It is used for unbounded strings"), and it is
/// kept as zero rather than mapped to `u32::MAX` so that
/// [`bound_exceeds`] — which already reads zero on either side the way
/// the clause does — stays the single place that knows it.
fn key_length_of(ti: &TypeIdentifier) -> Option<u32> {
    match ti {
        TypeIdentifier::String8Small { .. } | TypeIdentifier::String8Large { .. } => {
            Some(string_bound_u32_s8(ti))
        }
        TypeIdentifier::String16Small { .. } | TypeIdentifier::String16Large { .. } => {
            Some(string_bound_u32_s16(ti))
        }
        TypeIdentifier::PlainSequenceSmall { .. }
        | TypeIdentifier::PlainSequenceLarge { .. }
        | TypeIdentifier::PlainMapSmall { .. }
        | TypeIdentifier::PlainMapLarge { .. } => Some(collection_bound_of(ti)),
        _ => None,
    }
}

/// The bound of a plain sequence or plain map.
fn collection_bound_of(ti: &TypeIdentifier) -> u32 {
    match ti {
        TypeIdentifier::PlainSequenceSmall { .. } | TypeIdentifier::PlainSequenceLarge { .. } => {
            sequence_parts(ti).1
        }
        TypeIdentifier::PlainMapSmall { .. } | TypeIdentifier::PlainMapLarge { .. } => {
            map_parts(ti).2
        }
        _ => 0,
    }
}

/// XTypes 1.3 Table 19, `STRUCTURE_TYPE`, the three key-member bullets
/// that read a *length* or a *literal set* rather than a type:
///
/// > - For any string key member m2 in T2, the m1 member of T1 with the
/// >   same member ID verifies `m1.type.length >= m2.type.length`.
/// > - For any enumerated key member m2 in T2, the m1 member of T1 with
/// >   the same member ID verifies that all literals in m2.type appear
/// >   as literals in m1.type.
/// > - For any sequence or map key member m2 in T2, the m1 member of T1
/// >   with the same member ID verifies
/// >   `m1.type.length >= m2.type.length`.
///
/// and the note the table closes with: "The rules regarding key members
/// ensure that the key of T2 can be transformed faithfully into the key
/// of T1 without aliasing or loss of information."
///
/// That note is what each of the three costs when it is missing. A
/// reader whose `@key string<8>` matches a writer's `@key string<16>`
/// receives keys it cannot hold: two distinct instances whose first
/// eight octets agree become one instance, and the samples of one
/// overwrite the samples of the other under `KEEP_LAST(1)`. The same for
/// a sequence bound and for an enumerated literal the reader has no name
/// for.
///
/// `m2` is the **writer**'s member and `m1` the **reader**'s: XTypes
/// writes the relation as "T1 is-assignable-from T2" with T1 the reader,
/// and this function's arguments are `(w, r)` in the crate's own order.
///
/// The general "type is assignable" rule that `check_member_pair` runs
/// does not cover these: a `string<8>` reader *is* assignable from a
/// `string<16>` writer for an ordinary member — the writer's samples get
/// TRIMmed or DISCARDed per `@try_construct` — and it is only being a
/// key that makes the loss inadmissible.
fn key_members_fit(
    w: &crate::type_object::minimal::MinimalStructType,
    r: &crate::type_object::minimal::MinimalStructType,
    registry: &TypeRegistry,
    cfg: &AssignabilityConfig,
) -> Assignable {
    use crate::type_object::flags::StructMemberFlag;
    if cfg.key_member_bounds == KeyMemberBounds::Ignored {
        // The axis: the ordinary member rule already ran in
        // `check_member_pair`, and the three bullets are what this
        // function adds. See `KeyMemberBounds`.
        return Assignable::Yes;
    }
    for wm in &w.member_seq {
        if !wm.common.member_flags.has(StructMemberFlag::IS_KEY) {
            continue;
        }
        let Some(rm) = r
            .member_seq
            .iter()
            .find(|rm| rm.common.member_id == wm.common.member_id)
        else {
            // "Members marked as key in either T1 or T2 appear ... in
            // both T1 and T2" — the bullet above these three. The
            // extensibility walks enforce it through
            // `must_not_be_missing`, which folds `IS_KEY` in; nothing to
            // add here.
            continue;
        };
        // §7.2.4.4.3: the rules are about the type an alias names, not
        // about the alias.
        let (Ok(wt), Ok(rt)) = (
            resolve_alias_chain(&wm.common.member_type_id, registry, cfg.max_depth),
            resolve_alias_chain(&rm.common.member_type_id, registry, cfg.max_depth),
        ) else {
            continue;
        };

        // The two length bullets. `bound_exceeds(writer, reader)` is the
        // same predicate the ordinary string rule uses, and it reads a
        // zero bound on either side as "unbounded" the way Table 29 does.
        if let (Some(wl), Some(rl)) = (key_length_of(&wt), key_length_of(&rt))
            && bound_exceeds(wl, rl)
        {
            return Assignable::No(
                "Table 19: a key member of the reader is shorter than the writer's, \
                 so the writer's key cannot be held without loss",
            );
        }

        // The enumerated bullet. Every literal *value* the writer can
        // send has to be one the reader names, or the reader stores a
        // key it cannot tell apart from another.
        if let (Some(MinimalTypeObject::Enumerated(we)), Some(MinimalTypeObject::Enumerated(re))) = (
            resolve_minimal(&wt, registry),
            resolve_minimal(&rt, registry),
        ) {
            for wl in &we.literal_seq {
                if !re
                    .literal_seq
                    .iter()
                    .any(|rl| rl.common.value == wl.common.value)
                {
                    return Assignable::No(
                        "Table 19: a literal of the writer's enumerated key member is not \
                         a literal of the reader's",
                    );
                }
            }
        }

        // The structure and union bullets: `KeyHolder(m1.type)` is
        // assignable from `KeyHolder(m2.type)`, and for a union also the
        // per-discriminator-value rule.
        match key_holder_assignable(&wt, &rt, registry, cfg) {
            Assignable::Yes => {}
            e => return e,
        }
        if let (Some(MinimalTypeObject::Union(wu)), Some(MinimalTypeObject::Union(ru))) = (
            resolved_minimal_object(&wt, registry, cfg),
            resolved_minimal_object(&rt, registry, cfg),
        ) {
            match union_key_values_fit(&wu, &ru, registry, cfg) {
                Assignable::Yes => {}
                e => return e,
            }
        }
    }
    Assignable::Yes
}

/// `KeyHolder(T)` of a structure (XTypes 1.3 §7.2.2.4.7): when `T` has
/// key members, `T` without the members that are not keys; when it has
/// none, `T` with a key designation on every member. The members of a
/// base type are expanded into `T` first (Table 19, last paragraph).
fn key_holder_struct(
    s: &crate::type_object::minimal::MinimalStructType,
    registry: &TypeRegistry,
    cfg: &AssignabilityConfig,
) -> Option<crate::type_object::minimal::MinimalStructType> {
    use crate::type_object::flags::StructMemberFlag;
    let mut flat = if s.header.base_type == TypeIdentifier::None {
        s.clone()
    } else {
        flatten_inheritance(s, registry, cfg.max_depth).ok()?
    };
    if flat
        .member_seq
        .iter()
        .any(|m| m.common.member_flags.has(StructMemberFlag::IS_KEY))
    {
        flat.member_seq
            .retain(|m| m.common.member_flags.has(StructMemberFlag::IS_KEY));
    } else {
        for m in &mut flat.member_seq {
            m.common.member_flags.0 |= StructMemberFlag::IS_KEY;
        }
    }
    Some(flat)
}

/// The values a union discriminator of this type can take, where that set
/// is small enough to be listed: `boolean` and an enumeration. `None` for
/// every other discriminator type, whose values are taken to be more than
/// any union names.
fn discriminator_values(
    ti: &TypeIdentifier,
    registry: &TypeRegistry,
    cfg: &AssignabilityConfig,
) -> Option<alloc::collections::BTreeSet<i32>> {
    let resolved = resolve_alias_chain(ti, registry, cfg.max_depth).ok()?;
    match &resolved {
        TypeIdentifier::Primitive(PrimitiveKind::Boolean) => Some([0, 1].into_iter().collect()),
        TypeIdentifier::EquivalenceHashMinimal(_) | TypeIdentifier::EquivalenceHashComplete(_) => {
            match resolve_minimal(&resolved, registry)? {
                MinimalTypeObject::Enumerated(e) => {
                    Some(e.literal_seq.iter().map(|l| l.common.value).collect())
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// `KeyHolder(m1.type)` is-assignable-from `KeyHolder(m2.type)`, for the
/// type of a key member (Table 19: "For any structure or union key member
/// m2 in T2"). `w` is the type of `m2` and `r` the type of `m1`. A type
/// that is neither a structure nor a union is its own key holder.
fn key_holder_assignable(
    w: &TypeIdentifier,
    r: &TypeIdentifier,
    registry: &TypeRegistry,
    cfg: &AssignabilityConfig,
) -> Assignable {
    if w == r {
        return Assignable::Yes;
    }
    match (
        resolved_minimal_object(w, registry, cfg),
        resolved_minimal_object(r, registry, cfg),
    ) {
        (Some(MinimalTypeObject::Struct(ws)), Some(MinimalTypeObject::Struct(rs))) => {
            let (Some(wh), Some(rh)) = (
                key_holder_struct(&ws, registry, cfg),
                key_holder_struct(&rs, registry, cfg),
            ) else {
                return Assignable::No("a key member's base type does not flatten");
            };
            check_minimal_types(
                &MinimalTypeObject::Struct(wh),
                &MinimalTypeObject::Struct(rh),
                registry,
                cfg,
            )
        }
        (Some(MinimalTypeObject::Union(wu)), Some(MinimalTypeObject::Union(ru))) => {
            use crate::type_object::flags::UnionDiscriminatorFlag;
            let is_key = |u: &crate::type_object::minimal::MinimalUnionType| {
                u.discriminator
                    .common
                    .member_flags
                    .has(UnionDiscriminatorFlag::IS_KEY)
            };
            if is_key(&wu) || is_key(&ru) {
                // KeyHolder of a union with a key discriminator is the
                // discriminator alone (§7.2.2.4.7).
                if is_key(&wu) != is_key(&ru) {
                    return Assignable::No(
                        "union key member: one discriminator is a key and the other is not",
                    );
                }
                strongly_assignable(
                    &wu.discriminator.common.type_id,
                    &ru.discriminator.common.type_id,
                    registry,
                    cfg,
                )
            } else {
                // Without a key discriminator KeyHolder(T) is T.
                check_minimal_types(
                    &MinimalTypeObject::Union(wu),
                    &MinimalTypeObject::Union(ru),
                    registry,
                    cfg,
                )
            }
        }
        _ => is_assignable(w, r, registry, cfg),
    }
}

/// Table 19, the last key bullet: "For any union key member m2 in T2, the
/// m1 member of T1 with the same member ID verifies that: For every
/// discriminator value of m2.type that selects a member m22 in m2.type,
/// the discriminator value selects a member m11 in m1.type that verifies
/// KeyHolder(m11.type) is-assignable-from KeyHolder(m22.type)."
///
/// `w` is `m2.type` and `r` is `m1.type`. The values are the explicit
/// labels of `w`, and for the values `w` leaves to its default member the
/// labels of `r` that `w` does not name, then whatever neither names when
/// the discriminator has such values (a list is kept for `boolean` and
/// enumerations; any other discriminator type is taken to have them).
fn union_key_values_fit(
    w: &crate::type_object::minimal::MinimalUnionType,
    r: &crate::type_object::minimal::MinimalUnionType,
    registry: &TypeRegistry,
    cfg: &AssignabilityConfig,
) -> Assignable {
    use crate::type_object::flags::UnionMemberFlag;
    use crate::type_object::minimal::MinimalUnionMember;
    let is_default =
        |m: &&MinimalUnionMember| m.common.member_flags.0 & UnionMemberFlag::IS_DEFAULT != 0;
    let w_default = w.member_seq.iter().find(is_default);
    let r_default = r.member_seq.iter().find(is_default);
    let by_label = |u: &'_ crate::type_object::minimal::MinimalUnionType, v: i32| {
        u.member_seq
            .iter()
            .find(|m| m.common.label_seq.contains(&v))
            .map(|m| m.common.type_id.clone())
    };
    let w_labels: alloc::collections::BTreeSet<i32> = w
        .member_seq
        .iter()
        .flat_map(|m| m.common.label_seq.iter().copied())
        .collect();
    let r_labels: alloc::collections::BTreeSet<i32> = r
        .member_seq
        .iter()
        .flat_map(|m| m.common.label_seq.iter().copied())
        .collect();

    for v in &w_labels {
        let Some(w_type) = by_label(w, *v) else {
            continue;
        };
        let Some(r_type) = by_label(r, *v).or_else(|| r_default.map(|d| d.common.type_id.clone()))
        else {
            return Assignable::No(
                "Table 19: a discriminator value of the writer's union key member selects no \
                 member of the reader's",
            );
        };
        match key_holder_assignable(&w_type, &r_type, registry, cfg) {
            Assignable::Yes => {}
            e => return e,
        }
    }
    if let Some(wd) = w_default {
        let domain = discriminator_values(&w.discriminator.common.type_id, registry, cfg);
        for v in r_labels.difference(&w_labels) {
            if domain.as_ref().is_some_and(|d| !d.contains(v)) {
                continue;
            }
            let Some(r_type) = by_label(r, *v) else {
                continue;
            };
            match key_holder_assignable(&wd.common.type_id, &r_type, registry, cfg) {
                Assignable::Yes => {}
                e => return e,
            }
        }
        let left_over = match &domain {
            Some(d) => d
                .iter()
                .any(|v| !w_labels.contains(v) && !r_labels.contains(v)),
            None => true,
        };
        if left_over {
            let Some(rd) = r_default else {
                return Assignable::No(
                    "Table 19: the writer's union key member selects its default member for \
                     values the reader's union has no member for",
                );
            };
            match key_holder_assignable(&wd.common.type_id, &rd.common.type_id, registry, cfg) {
                Assignable::Yes => {}
                e => return e,
            }
        }
    }
    Assignable::Yes
}

/// `prevent_type_widening` for an aggregate (XTypes 1.3 §7.6.3.4.1).
///
/// The reader may not declare a member the writer does not have. What
/// the flag forbids is precisely the tolerance the extensibility rules
/// grant: an `@appendable` reader whose member sequence is longer than
/// the writer's reads the tail as its default value, and a `@mutable`
/// reader may carry ids the writer never sends. Both are "the reader
/// type is wider than the writer type", and both are refused when the
/// reader asked for that.
///
/// A `@final` pair cannot widen — §7.2.4.4.4.4 already demands the same
/// set of member ids — so the check costs it nothing.
fn reader_is_not_wider(
    w: &crate::type_object::minimal::MinimalStructType,
    r: &crate::type_object::minimal::MinimalStructType,
) -> Assignable {
    for rm in &r.member_seq {
        let known = w
            .member_seq
            .iter()
            .any(|wm| wm.common.member_id == rm.common.member_id);
        if !known {
            return Assignable::No(
                "prevent_type_widening: the reader declares a member the writer does not",
            );
        }
    }
    Assignable::Yes
}

/// §7.2.4.4.4.4, `FINAL`: the two member sequences are the same one.
///
/// No evolution at all is the point of `@final`, and the corpus says so
/// in bytes: `evolution-final-tail` adds one member to a `@final` struct
/// and the reader refuses the payload in **every** encoding and both
/// byte orders.
fn check_final_structs(
    w: &crate::type_object::minimal::MinimalStructType,
    r: &crate::type_object::minimal::MinimalStructType,
    registry: &TypeRegistry,
    cfg: &AssignabilityConfig,
) -> Assignable {
    if w.member_seq.len() != r.member_seq.len() {
        return Assignable::No("final struct member count mismatch");
    }
    for (wm, rm) in w.member_seq.iter().zip(r.member_seq.iter()) {
        if wm.common.member_id != rm.common.member_id {
            return Assignable::No("final struct member ids differ");
        }
        match check_member_pair(
            wm,
            rm,
            registry,
            cfg,
            "final struct member name-hash differs",
            true,
        ) {
            Assignable::Yes => {}
            e => return e,
        }
    }
    Assignable::Yes
}

/// §7.2.4.4.4.4, `APPENDABLE`: the shorter member sequence is a prefix
/// of the longer.
///
/// **Either** side may be the longer one, and that is what this rebuild
/// corrected. The `cdr`-era implementation refused the pair whenever the
/// reader had more members than the writer — "the writer must have at
/// least all reader fields as a prefix" — which is not what the clause
/// says and not what the bytes do. `evolution-appendable-tail` is a
/// two-member writer and a three-member reader; the recorded rows have
/// the reader reading the payload in every encoding and both byte
/// orders, with the added `short c` taking its default. A rule that
/// called that pair incompatible would have a discovery refuse a match
/// that works, which is the failure `@appendable` exists to prevent.
///
/// The defect was invisible from inside the crate: every test it had
/// built both types itself and none of them was a writer that had not
/// yet grown a member.
fn check_appendable_structs(
    w: &crate::type_object::minimal::MinimalStructType,
    r: &crate::type_object::minimal::MinimalStructType,
    registry: &TypeRegistry,
    cfg: &AssignabilityConfig,
) -> Assignable {
    let shared = w.member_seq.len().min(r.member_seq.len());
    for (wm, rm) in w.member_seq.iter().zip(r.member_seq.iter()).take(shared) {
        if wm.common.member_id != rm.common.member_id {
            return Assignable::No("appendable: the shared prefix has different member ids");
        }
        match check_member_pair(
            wm,
            rm,
            registry,
            cfg,
            "appendable: member name-hash differs",
            true,
        ) {
            Assignable::Yes => {}
            e => return e,
        }
    }
    // Past the prefix, only a member the other side must not do without
    // stops the pair.
    for tail in w.member_seq.iter().skip(shared) {
        if must_not_be_missing(tail.common.member_flags) {
            return Assignable::No(
                "appendable: a writer member past the prefix is must-understand",
            );
        }
    }
    for tail in r.member_seq.iter().skip(shared) {
        if must_not_be_missing(tail.common.member_flags) {
            return Assignable::No(
                "appendable: a reader member past the prefix is must-understand",
            );
        }
    }
    Assignable::Yes
}

/// §7.2.4.4.4.4, `MUTABLE`: members are matched by id and order does not
/// enter into it.
///
/// What decides a member present on only one side is
/// [`must_not_be_missing`] and **not** whether it is `@optional`. The
/// `cdr`-era implementation used optionality, which refused every pair
/// where the reader had grown an ordinary member — `evolution-mutable-
/// tail`, `evolution-mutable-middle`, `evolution-mutable-map` and five
/// others, all of which the recorded rows have reading successfully in
/// every encoding. `@optional` decides whether a member's *absence from
/// a sample* is legal; `must_understand` decides whether its absence
/// from the *type* is. The corpus separates the two: `evolution-mutable-
/// optional` and `evolution-mutable-mustunderstand` differ in exactly
/// that annotation on the added member.
///
/// The other direction was not checked at all, which is the silent half:
/// a writer that marks a member `@must_understand` and a reader that
/// does not declare it were called assignable, and §7.2.4.4.4.4 makes
/// them not.
fn check_mutable_structs(
    w: &crate::type_object::minimal::MinimalStructType,
    r: &crate::type_object::minimal::MinimalStructType,
    registry: &TypeRegistry,
    cfg: &AssignabilityConfig,
) -> Assignable {
    for rm in &r.member_seq {
        match w
            .member_seq
            .iter()
            .find(|wm| wm.common.member_id == rm.common.member_id)
        {
            Some(wm) => {
                match check_member_pair(
                    wm,
                    rm,
                    registry,
                    cfg,
                    "mutable: member name-hash differs despite id match",
                    false,
                ) {
                    Assignable::Yes => {}
                    e => return e,
                }
            }
            None if must_not_be_missing(rm.common.member_flags) => {
                return Assignable::No(
                    "mutable: a must-understand reader member is not in the writer",
                );
            }
            None => {}
        }
    }
    for wm in &w.member_seq {
        let known = r
            .member_seq
            .iter()
            .any(|rm| rm.common.member_id == wm.common.member_id);
        if !known && must_not_be_missing(wm.common.member_flags) {
            return Assignable::No("mutable: a must-understand writer member is not in the reader");
        }
    }
    Assignable::Yes
}

/// XTypes 1.3 §7.2.4.4.4.5 — union types.
///
/// Three things decide it, and the third is what makes a union
/// different from a structure. The extensibility kinds have to agree,
/// as for every aggregate. The discriminators have to be assignable,
/// because the discriminator is read before anything else and a reader
/// that read it at the wrong width would be lost for the rest of the
/// sample. And a label that appears on both sides has to select **the
/// same member** on both: a writer that sends `case 2` meaning `short b`
/// to a reader whose `case 2` means `string s` has sent a member the
/// reader will decode as something else, which is the silent shape.
///
/// A label only the writer has is not a refusal for an `@appendable` or
/// `@mutable` union — Table 19's `UNION_TYPE` row says what happens
/// then: "If the discriminator value does not select any member in O2,
/// then: If the discriminator value selects a member m1 in O1, then m1
/// is set to its default value according to its type." A `@final` union
/// has no such tolerance, so its label sets have to be equal.
fn check_unions(
    w: &crate::type_object::minimal::MinimalUnionType,
    r: &crate::type_object::minimal::MinimalUnionType,
    registry: &TypeRegistry,
    cfg: &AssignabilityConfig,
) -> Assignable {
    use crate::type_object::flags::StructTypeFlag;

    let extensibility = |flags: crate::type_object::flags::UnionTypeFlag| {
        if flags.0 & StructTypeFlag::IS_FINAL != 0 {
            StructExt::Final
        } else if flags.0 & StructTypeFlag::IS_MUTABLE != 0 {
            StructExt::Mutable
        } else {
            StructExt::Appendable
        }
    };
    let w_ext = extensibility(w.union_flags);
    if w_ext != extensibility(r.union_flags) {
        return Assignable::No("extensibility mismatch");
    }

    // §7.2.4.4.4.5's first condition, and the one this used to skip.
    // Table 19's `UNION_TYPE` row states it as the third bullet:
    //
    // > Either the discriminators of both T1 and T2 are keys or neither
    // > are keys
    //
    // A keyed discriminator makes the union an instance-carrying type
    // and an unkeyed one does not, so a pair that disagrees has one side
    // splitting the samples into instances and the other holding them
    // all as one. It is checked before the discriminator types are
    // compared, because it holds whatever the two types are — a `uint32`
    // key discriminator is not assignable to a `uint32` one that is not
    // a key, and the type comparison would say `Yes` to that pair.
    //
    // `UnionDiscriminatorFlag::IS_KEY` is Annex B's slot for it. It is set
    // for a discriminator written `@key`, which the front end carries
    // unless it was asked to refuse it —
    // `zerodds_idl_lower::KeyOnUnionDiscriminator::Refused`. With the
    // flag missing altogether the rule had nothing to compare and every
    // such pair matched.
    // The OMG suite's `union_uint32_one_key` is what it is for: two
    // declarations that differ in nothing else and must not communicate.
    let is_key = |flags: crate::type_object::flags::UnionDiscriminatorFlag| {
        flags.has(crate::type_object::flags::UnionDiscriminatorFlag::IS_KEY)
    };
    if is_key(w.discriminator.common.member_flags) != is_key(r.discriminator.common.member_flags) {
        return Assignable::No(
            "union: one discriminator is a key and the other is not (§7.2.4.4.4.5, Table 19)",
        );
    }

    match strongly_assignable(
        &w.discriminator.common.type_id,
        &r.discriminator.common.type_id,
        registry,
        cfg,
    ) {
        Assignable::Yes => {}
        e => return e,
    }

    if !cfg.ignore_member_names {
        match union_names_and_ids_agree(w, r) {
            Assignable::Yes => {}
            e => return e,
        }
    }
    if cfg.prevent_type_widening {
        for rm in &r.member_seq {
            for label in &rm.common.label_seq {
                let known = w
                    .member_seq
                    .iter()
                    .any(|wm| wm.common.label_seq.contains(label));
                if !known {
                    return Assignable::No(
                        "prevent_type_widening: the reader declares a case label the writer does not",
                    );
                }
            }
        }
    }

    // The default member, which is a member with no label to be found
    // by. `IS_DEFAULT` is the flag §7.3.4.5 gives it and the label
    // `default` maps to no numeric value, so its `label_seq` is empty
    // and every label loop below skips it.
    let default_of = |u: &crate::type_object::minimal::MinimalUnionType| {
        u.member_seq
            .iter()
            .find(|m| {
                m.common.member_flags.0 & crate::type_object::flags::UnionMemberFlag::IS_DEFAULT
                    != 0
            })
            .cloned()
    };
    let w_default = default_of(w);
    let r_default = default_of(r);

    // §7.2.4.4.4.5, the non-final rule: "Either the discriminators of
    // both T1 and T2 are keys or neither are keys[, and] there is at
    // least one member m1 of T1 and one member m2 of T2 with the same
    // case label."
    //
    // The `default:` case does not supply one. Its label is not a
    // discriminator value at all — it is "whatever is left" — and two
    // unions whose only agreement is that each has a default have no
    // value on which they mean the same thing. The suite separates the
    // two shapes on purpose: `union_appendable_one_common_1` shares
    // label 5 and communicates, `union_appendable_no_common_1` shares
    // nothing and must not, and `union_appendable_no_common_w_default`
    // is the pair whose only common ground is the default member.
    //
    // A `@final` union is covered by the stricter equality of the label
    // sets below.
    if !matches!(w_ext, StructExt::Final) {
        let common = w.member_seq.iter().any(|wm| {
            wm.common.label_seq.iter().any(|label| {
                r.member_seq
                    .iter()
                    .any(|rm| rm.common.label_seq.contains(label))
            })
        });
        if !common {
            return Assignable::No("union: the two types have no case label in common");
        }
    }

    for wm in &w.member_seq {
        for label in &wm.common.label_seq {
            let Some(rm) = r
                .member_seq
                .iter()
                .find(|rm| rm.common.label_seq.contains(label))
            else {
                // The reader does not declare this label. Final admits
                // no such gap; the other two default the member.
                if matches!(w_ext, StructExt::Final) {
                    return Assignable::No("final union: writer label unknown in reader");
                }
                // §7.2.4.4.4.5: a discriminator value that selects no
                // explicit member of the reader selects the reader's
                // *default* member, if it has one — so that is the
                // member the writer's has to be assignable to. Skipping
                // it let a writer send `case 2: short` to a reader whose
                // `default: long` is what value 2 lands in, and the
                // reader decoded four octets where two were sent. The
                // suite's `union_int16_int32_default`, against
                // `union_int32_int32_default`, which is the same shape
                // with the widths agreeing.
                if let Some(rd) = r_default.as_ref() {
                    match is_assignable(&wm.common.type_id, &rd.common.type_id, registry, cfg) {
                        Assignable::Yes => {}
                        e => return e,
                    }
                }
                continue;
            };
            // No member-id comparison here. A union member's id is
            // assigned by declaration order unless the model writes one,
            // so `union_1 { case 1: x1; case 2: x2; }` and
            // `union_2 { case 2: x2; case 1: x1; }` — the same three
            // cases in a different order — carry different ids for the
            // same label and were refused as "one label selects
            // different members". What §7.2.4.4.4.5 asks about a shared
            // label is that the two selected members' *types* relate,
            // which is the call below; the id/name agreement is the
            // separate cross-check above, and `ignore_member_names`
            // switches that one off.
            match is_assignable(&wm.common.type_id, &rm.common.type_id, registry, cfg) {
                Assignable::Yes => {}
                e => return e,
            }
        }
    }

    if matches!(w_ext, StructExt::Final) {
        // Equality of the label sets, which the loop above only checked
        // in one direction.
        for rm in &r.member_seq {
            for label in &rm.common.label_seq {
                let known = w
                    .member_seq
                    .iter()
                    .any(|wm| wm.common.label_seq.contains(label));
                if !known {
                    return Assignable::No("final union: reader label unknown in writer");
                }
            }
        }
    }

    // The mirror of the writer loop's default arm: a label the reader
    // declares explicitly and the writer does not is a value the
    // writer's *default* member carries, so the reader's member has to
    // be assignable from it. The suite's `union_int32_default_int16`.
    if let Some(wd) = w_default.as_ref() {
        for rm in &r.member_seq {
            for label in &rm.common.label_seq {
                let known = w
                    .member_seq
                    .iter()
                    .any(|wm| wm.common.label_seq.contains(label));
                if !known {
                    match is_assignable(&wd.common.type_id, &rm.common.type_id, registry, cfg) {
                        Assignable::Yes => {}
                        e => return e,
                    }
                }
            }
        }
    }

    match (w_default, r_default) {
        (Some(wd), Some(rd)) => {
            is_assignable(&wd.common.type_id, &rd.common.type_id, registry, cfg)
        }
        (None, None) => Assignable::Yes,
        // A writer with a default member can send a discriminator value
        // the reader has no case for; a reader with one absorbs it. The
        // asymmetry is the whole point of `default:` — and `@final`
        // has none of it: §7.2.4.4.4.5 makes the two member sets equal,
        // and a default member is a member. The suite runs both
        // directions, `union_final_one_default_1` and `_2`.
        (None, Some(_)) if matches!(w_ext, StructExt::Final) => {
            Assignable::No("final union: the reader has a default member and the writer has none")
        }
        (None, Some(_)) => Assignable::Yes,
        (Some(_), None) if matches!(w_ext, StructExt::Final) => {
            Assignable::No("final union: the writer has a default member and the reader has none")
        }
        (Some(_), None) => Assignable::Yes,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::builder::{Extensibility, TypeObjectBuilder};
    use crate::hash::compute_minimal_hash;
    use crate::type_object::TypeObject;

    #[test]
    fn primitive_same_kind_is_assignable() {
        let reg = TypeRegistry::new();
        let a = is_assignable(
            &TypeIdentifier::Primitive(PrimitiveKind::Int32),
            &TypeIdentifier::Primitive(PrimitiveKind::Int32),
            &reg,
            &AssignabilityConfig::default(),
        );
        assert!(a.is_yes());
    }

    #[test]
    fn primitive_different_kind_is_not_assignable_by_default() {
        let reg = TypeRegistry::new();
        let a = is_assignable(
            &TypeIdentifier::Primitive(PrimitiveKind::Int32),
            &TypeIdentifier::Primitive(PrimitiveKind::Int64),
            &reg,
            &AssignabilityConfig::default(),
        );
        assert!(!a.is_yes());
    }

    /// XTypes 1.3 §7.2.4.4.4, Table 15: "Any Primitive Type" is
    /// assignable from "The same Primitive Type" and from nothing else.
    /// Widening is not a row of that table, so `allow_type_coercion` does
    /// not buy it — in either direction.
    #[test]
    fn primitive_widening_is_not_assignable_even_with_coercion() {
        let reg = TypeRegistry::new();
        let cfg = AssignabilityConfig {
            allow_type_coercion: true,
            ..Default::default()
        };
        assert!(
            !is_assignable(
                &TypeIdentifier::Primitive(PrimitiveKind::Int32),
                &TypeIdentifier::Primitive(PrimitiveKind::Int64),
                &reg,
                &cfg,
            )
            .is_yes(),
            "int32 → int64 is not a row of Table 15"
        );
        // Narrowing was already forbidden and stays forbidden.
        assert!(
            !is_assignable(
                &TypeIdentifier::Primitive(PrimitiveKind::Int64),
                &TypeIdentifier::Primitive(PrimitiveKind::Int32),
                &reg,
                &cfg,
            )
            .is_yes()
        );
        // The one row that is in the table.
        assert!(
            is_assignable(
                &TypeIdentifier::Primitive(PrimitiveKind::Int64),
                &TypeIdentifier::Primitive(PrimitiveKind::Int64),
                &reg,
                &cfg,
            )
            .is_yes()
        );
    }

    #[test]
    fn appendable_struct_with_extra_writer_field_is_assignable() {
        let mut reg = TypeRegistry::new();
        let writer = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Appendable)
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .member("b", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .build_minimal(),
        );
        let reader = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Appendable)
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .build_minimal(),
        );
        let wh = compute_minimal_hash(&writer).unwrap();
        let rh = compute_minimal_hash(&reader).unwrap();
        reg.insert_minimal(wh, writer.clone());
        reg.insert_minimal(rh, reader);

        assert!(
            is_assignable(
                &TypeIdentifier::EquivalenceHashMinimal(wh),
                &TypeIdentifier::EquivalenceHashMinimal(rh),
                &reg,
                &AssignabilityConfig::default(),
            )
            .is_yes()
        );
    }

    #[test]
    fn final_struct_with_extra_writer_field_is_not_assignable() {
        let mut reg = TypeRegistry::new();
        let writer = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Final)
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .member("b", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .build_minimal(),
        );
        let reader = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Final)
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .build_minimal(),
        );
        let wh = compute_minimal_hash(&writer).unwrap();
        let rh = compute_minimal_hash(&reader).unwrap();
        reg.insert_minimal(wh, writer);
        reg.insert_minimal(rh, reader);

        assert!(
            !is_assignable(
                &TypeIdentifier::EquivalenceHashMinimal(wh),
                &TypeIdentifier::EquivalenceHashMinimal(rh),
                &reg,
                &AssignabilityConfig::default(),
            )
            .is_yes()
        );
    }

    #[test]
    fn mutable_struct_member_id_matching() {
        let mut reg = TypeRegistry::new();
        // Reader has @id(1) and @id(2) — writer has @id(2) and @id(3).
        // Reader @id(1) is missing in the writer, but we mark it as optional.
        let writer = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Mutable)
                .member("b", TypeIdentifier::Primitive(PrimitiveKind::Int64), |m| {
                    m.id(2)
                })
                .member("c", TypeIdentifier::Primitive(PrimitiveKind::Int64), |m| {
                    m.id(3)
                })
                .build_minimal(),
        );
        let reader = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Mutable)
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int64), |m| {
                    m.id(1).optional()
                })
                .member("b", TypeIdentifier::Primitive(PrimitiveKind::Int64), |m| {
                    m.id(2)
                })
                .build_minimal(),
        );
        let wh = compute_minimal_hash(&writer).unwrap();
        let rh = compute_minimal_hash(&reader).unwrap();
        reg.insert_minimal(wh, writer);
        reg.insert_minimal(rh, reader);

        assert!(
            is_assignable(
                &TypeIdentifier::EquivalenceHashMinimal(wh),
                &TypeIdentifier::EquivalenceHashMinimal(rh),
                &reg,
                &AssignabilityConfig::default(),
            )
            .is_yes()
        );
    }

    /// Cross-vendor reality (measured against CycloneDDS on the #28 gate):
    /// the peer announces the MINIMAL `TypeIdentifier` of its
    /// PID_TYPE_INFORMATION while the local typed endpoint carries the
    /// COMPLETE one for its own generated type. XTypes 1.3 §7.3.4.5 derives
    /// both forms from one type model, so the mixed pair decides exactly as
    /// the minimal/minimal pair does — the complete side is projected down
    /// and compared structurally.
    ///
    /// Before the mixed arms existed, this pair fell through to the
    /// `_ => "kinds do not match"` row: two structurally identical
    /// `@appendable` structs were reported inconsistent, the endpoints never
    /// matched, and no sample crossed. Deleting either arm restores that.
    #[test]
    fn minimal_and_complete_hash_of_the_same_struct_are_assignable() {
        let mut reg = TypeRegistry::new();
        let b = || {
            TypeObjectBuilder::struct_type("::Robot")
                .extensibility(Extensibility::Appendable)
                .member(
                    "id",
                    TypeIdentifier::Primitive(PrimitiveKind::UInt32),
                    |m| m.id(0),
                )
                .member(
                    "label",
                    TypeIdentifier::Primitive(PrimitiveKind::UInt32),
                    |m| m.id(1),
                )
        };
        let minimal = MinimalTypeObject::Struct(b().build_minimal());
        let complete =
            crate::type_object::complete::CompleteTypeObject::Struct(b().build_complete());
        let mh = compute_minimal_hash(&minimal).unwrap();
        let ch = crate::hash::compute_complete_hash(&complete).unwrap();
        reg.insert_minimal(mh, minimal);
        reg.insert_complete(ch, complete);

        // The two hashes differ by construction, so neither the equal-hash
        // shortcut nor the two same-form arms can answer this pair.
        assert_ne!(mh, ch, "minimal and complete hash must differ");

        // Peer minimal -> local complete, the direction the gate measured.
        assert!(
            is_assignable(
                &TypeIdentifier::EquivalenceHashMinimal(mh),
                &TypeIdentifier::EquivalenceHashComplete(ch),
                &reg,
                &AssignabilityConfig::default(),
            )
            .is_yes(),
            "a minimal writer type must be assignable to the complete form of the same type"
        );
        // And the mirrored direction, which the writer-side match takes.
        assert!(
            is_assignable(
                &TypeIdentifier::EquivalenceHashComplete(ch),
                &TypeIdentifier::EquivalenceHashMinimal(mh),
                &reg,
                &AssignabilityConfig::default(),
            )
            .is_yes(),
            "a complete writer type must be assignable to the minimal form of the same type"
        );
    }

    /// The mixed arms must decide, not wave through: an `@appendable`
    /// reader is not assignable from a `@final` writer (§7.2.4.4 — the two
    /// extensibility kinds differ), and that verdict must survive the
    /// minimal/complete projection unchanged.
    #[test]
    fn minimal_and_complete_hash_still_reject_an_extensibility_mismatch() {
        let mut reg = TypeRegistry::new();
        let writer = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::Robot")
                .extensibility(Extensibility::Final)
                .member(
                    "id",
                    TypeIdentifier::Primitive(PrimitiveKind::UInt32),
                    |m| m.id(0),
                )
                .build_minimal(),
        );
        let reader = crate::type_object::complete::CompleteTypeObject::Struct(
            TypeObjectBuilder::struct_type("::Robot")
                .extensibility(Extensibility::Appendable)
                .member(
                    "id",
                    TypeIdentifier::Primitive(PrimitiveKind::UInt32),
                    |m| m.id(0),
                )
                .build_complete(),
        );
        let wh = compute_minimal_hash(&writer).unwrap();
        let rh = crate::hash::compute_complete_hash(&reader).unwrap();
        reg.insert_minimal(wh, writer);
        reg.insert_complete(rh, reader);

        assert!(
            !is_assignable(
                &TypeIdentifier::EquivalenceHashMinimal(wh),
                &TypeIdentifier::EquivalenceHashComplete(rh),
                &reg,
                &AssignabilityConfig::default(),
            )
            .is_yes(),
            "a final writer must not be assignable to an appendable reader across the forms"
        );
    }

    #[test]
    fn extensibility_mismatch_fails() {
        let mut reg = TypeRegistry::new();
        let writer = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Final)
                .build_minimal(),
        );
        let reader = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Mutable)
                .build_minimal(),
        );
        let wh = compute_minimal_hash(&writer).unwrap();
        let rh = compute_minimal_hash(&reader).unwrap();
        reg.insert_minimal(wh, writer);
        reg.insert_minimal(rh, reader);

        let a = is_assignable(
            &TypeIdentifier::EquivalenceHashMinimal(wh),
            &TypeIdentifier::EquivalenceHashMinimal(rh),
            &reg,
            &AssignabilityConfig::default(),
        );
        assert!(!a.is_yes());
    }

    #[test]
    fn string_small_and_large_interchangeable() {
        let reg = TypeRegistry::new();
        assert!(
            is_assignable(
                &TypeIdentifier::String8Small { bound: 64 },
                &TypeIdentifier::String8Large { bound: 100_000 },
                &reg,
                &AssignabilityConfig::default(),
            )
            .is_yes()
        );
    }

    // Silence unused import when only some Variants are matched.
    #[allow(dead_code)]
    fn _unused() -> TypeObject {
        TypeObject::Minimal(MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::dummy").build_minimal(),
        ))
    }

    // ---- Additional coverage for check_direct arms -----------------------

    use crate::type_identifier::PlainCollectionHeader;
    use alloc::boxed::Box;

    fn reg() -> TypeRegistry {
        TypeRegistry::new()
    }

    #[test]
    fn string8_vs_string16_not_assignable() {
        let a = is_assignable(
            &TypeIdentifier::String8Small { bound: 16 },
            &TypeIdentifier::String16Small { bound: 16 },
            &reg(),
            &AssignabilityConfig::default(),
        );
        assert!(!a.is_yes());
        assert!(matches!(a, Assignable::No(msg) if msg.contains("kinds")));
    }

    #[test]
    fn string16_small_and_large_interchangeable() {
        let a = is_assignable(
            &TypeIdentifier::String16Small { bound: 32 },
            &TypeIdentifier::String16Large { bound: 10_000 },
            &reg(),
            &AssignabilityConfig::default(),
        );
        assert!(a.is_yes());
    }

    #[test]
    fn identical_type_identifiers_short_circuit_yes() {
        // Exact equality path (`w == r`) in check_direct.
        let ti = TypeIdentifier::Primitive(PrimitiveKind::UInt32);
        let a = is_assignable(&ti, &ti, &reg(), &AssignabilityConfig::default());
        assert!(a.is_yes());
    }

    #[test]
    fn sequence_writer_bound_exceeds_reader_bound_is_no() {
        let w = TypeIdentifier::PlainSequenceSmall {
            header: PlainCollectionHeader::default(),
            bound: 20,
            element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
        };
        let r = TypeIdentifier::PlainSequenceSmall {
            header: PlainCollectionHeader::default(),
            bound: 10,
            element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
        };
        // `ignore_sequence_bounds=false` forces the bound check.
        let cfg = AssignabilityConfig {
            ignore_sequence_bounds: false,
            ..Default::default()
        };
        let a = is_assignable(&w, &r, &reg(), &cfg);
        assert!(!a.is_yes());
        assert!(matches!(a, Assignable::No(msg) if msg.contains("bound")));
    }

    /// With `ignore_sequence_bounds=true` (TCE default) we accept
    /// the sequence even though the writer bound > reader bound.
    #[test]
    fn sequence_bounds_ignored_when_policy_allows() {
        let w = TypeIdentifier::PlainSequenceSmall {
            header: PlainCollectionHeader::default(),
            bound: 20,
            element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
        };
        let r = TypeIdentifier::PlainSequenceSmall {
            header: PlainCollectionHeader::default(),
            bound: 10,
            element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
        };
        assert!(is_assignable(&w, &r, &reg(), &AssignabilityConfig::default()).is_yes());
    }

    #[test]
    fn sequence_reader_unbounded_accepts_any_writer_bound() {
        // reader bound=0 (unbounded) → writer bound check skipped.
        let w = TypeIdentifier::PlainSequenceSmall {
            header: PlainCollectionHeader::default(),
            bound: 200,
            element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int16)),
        };
        let r = TypeIdentifier::PlainSequenceSmall {
            header: PlainCollectionHeader::default(),
            bound: 0,
            element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int16)),
        };
        assert!(is_assignable(&w, &r, &reg(), &AssignabilityConfig::default()).is_yes());
    }

    #[test]
    fn sequence_elements_must_be_assignable() {
        // Same bound, but different incompatible element kinds.
        let w = TypeIdentifier::PlainSequenceSmall {
            header: PlainCollectionHeader::default(),
            bound: 5,
            element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
        };
        let r = TypeIdentifier::PlainSequenceSmall {
            header: PlainCollectionHeader::default(),
            bound: 5,
            element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Float64)),
        };
        assert!(!is_assignable(&w, &r, &reg(), &AssignabilityConfig::default()).is_yes());
    }

    #[test]
    fn nested_sequence_of_sequence_assignable_when_elements_match() {
        let inner = TypeIdentifier::PlainSequenceSmall {
            header: PlainCollectionHeader::default(),
            bound: 5,
            element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Byte)),
        };
        let outer = TypeIdentifier::PlainSequenceSmall {
            header: PlainCollectionHeader::default(),
            bound: 3,
            element: Box::new(inner.clone()),
        };
        // outer vs outer with inner bound differing: inner writer=5, reader=10 → OK
        let inner_wider = TypeIdentifier::PlainSequenceSmall {
            header: PlainCollectionHeader::default(),
            bound: 10,
            element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Byte)),
        };
        let outer2 = TypeIdentifier::PlainSequenceSmall {
            header: PlainCollectionHeader::default(),
            bound: 3,
            element: Box::new(inner_wider),
        };
        assert!(is_assignable(&outer, &outer2, &reg(), &AssignabilityConfig::default()).is_yes());
    }

    #[test]
    fn plain_array_identical_assigns_yes() {
        let a = is_assignable(
            &TypeIdentifier::PlainArraySmall {
                header: PlainCollectionHeader::default(),
                array_bounds: alloc::vec![3, 4],
                element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
            },
            &TypeIdentifier::PlainArraySmall {
                header: PlainCollectionHeader::default(),
                array_bounds: alloc::vec![3, 4],
                element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
            },
            &reg(),
            &AssignabilityConfig::default(),
        );
        assert!(a.is_yes());
    }

    #[test]
    fn plain_array_diff_bounds_is_no() {
        let b = is_assignable(
            &TypeIdentifier::PlainArraySmall {
                header: PlainCollectionHeader::default(),
                array_bounds: alloc::vec![3, 4],
                element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
            },
            &TypeIdentifier::PlainArraySmall {
                header: PlainCollectionHeader::default(),
                array_bounds: alloc::vec![3, 5],
                element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
            },
            &reg(),
            &AssignabilityConfig::default(),
        );
        assert!(!b.is_yes());
        assert!(matches!(b, Assignable::No(msg) if msg.contains("array bounds")));
    }

    #[test]
    fn equivalence_hash_identical_short_circuits_to_yes() {
        // For identical hashes, `check_direct` takes the early exit
        // `wh == rh → Yes` (see the match arm). The only precondition is
        // that `resolve_alias_chain` succeeds — for that the
        // hash must point to a non-alias in the registry.
        let mut reg = reg();
        let to = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::T")
                .extensibility(Extensibility::Appendable)
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .build_minimal(),
        );
        let h = compute_minimal_hash(&to).unwrap();
        reg.insert_minimal(h, to);
        let a = is_assignable(
            &TypeIdentifier::EquivalenceHashMinimal(h),
            &TypeIdentifier::EquivalenceHashMinimal(h),
            &reg,
            &AssignabilityConfig::default(),
        );
        assert!(a.is_yes());
    }

    #[test]
    fn equivalence_hash_unresolved_writer_is_no() {
        // Alias resolution fails because the hash is not in the
        // registry → early exit with `No("writer alias resolution failed")`.
        let reg = reg();
        let wh = crate::type_identifier::EquivalenceHash([0x01; 14]);
        let rh = crate::type_identifier::EquivalenceHash([0x02; 14]);
        let a = is_assignable(
            &TypeIdentifier::EquivalenceHashMinimal(wh),
            &TypeIdentifier::EquivalenceHashMinimal(rh),
            &reg,
            &AssignabilityConfig::default(),
        );
        assert!(!a.is_yes());
        assert!(matches!(a, Assignable::No(msg) if msg.contains("alias resolution")));
    }

    #[test]
    fn equivalence_hash_unresolved_reader_is_no() {
        // Writer is registered, reader is not → error on the reader resolve.
        let mut reg = reg();
        let to = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::T")
                .extensibility(Extensibility::Appendable)
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .build_minimal(),
        );
        let wh = compute_minimal_hash(&to).unwrap();
        reg.insert_minimal(wh, to);
        let rh = crate::type_identifier::EquivalenceHash([0x02; 14]);
        let a = is_assignable(
            &TypeIdentifier::EquivalenceHashMinimal(wh),
            &TypeIdentifier::EquivalenceHashMinimal(rh),
            &reg,
            &AssignabilityConfig::default(),
        );
        assert!(!a.is_yes());
        assert!(matches!(a, Assignable::No(msg) if msg.contains("reader")));
    }

    #[test]
    fn mixed_kinds_report_kinds_do_not_match() {
        // Primitive vs string → no arm matched → `No("kinds do not match")`.
        let a = is_assignable(
            &TypeIdentifier::Primitive(PrimitiveKind::Int32),
            &TypeIdentifier::String8Small { bound: 10 },
            &reg(),
            &AssignabilityConfig::default(),
        );
        assert!(!a.is_yes());
    }

    /// This test read `..._is_no` and asserted the opposite. It pinned
    /// the containment rule — "every writer literal must be a literal
    /// of the reader" — and §7.2.4.4.4.3 asks for agreement instead:
    /// "any literals that have the same name also have the same value
    /// and any literals that have the same value also have the same
    /// name". A literal only one side declares breaks neither.
    ///
    /// The consequence is not academic. `@try_construct` on an
    /// enumeration member (§7.2.2.7, Table 9's `ENUM_TYPE` row) exists
    /// for exactly the value the receiver's enumeration does not
    /// declare, and a reader that refused the *match* would never reach
    /// it. Seven of the suite's cases are that shape, `tryc_enum_1`
    /// through `struct_key_enum_2`.
    #[test]
    fn an_enum_that_grew_a_literal_is_still_assignable() {
        let mut reg = reg();
        let w = MinimalTypeObject::Enumerated(
            TypeObjectBuilder::enum_type("::E")
                .bit_bound(32)
                .literal("A", 1)
                .literal("B", 2)
                .build_minimal(),
        );
        let r = MinimalTypeObject::Enumerated(
            TypeObjectBuilder::enum_type("::E")
                .bit_bound(32)
                .literal("A", 1)
                .build_minimal(),
        );
        let wh = crate::hash::compute_minimal_hash(&w).unwrap();
        let rh = crate::hash::compute_minimal_hash(&r).unwrap();
        reg.insert_minimal(wh, w);
        reg.insert_minimal(rh, r);
        let w_id = TypeIdentifier::EquivalenceHashMinimal(wh);
        let r_id = TypeIdentifier::EquivalenceHashMinimal(rh);

        assert_eq!(
            is_assignable(&w_id, &r_id, &reg, &AssignabilityConfig::default()),
            Assignable::Yes,
            "the reader lacking B contradicts nothing the writer declares"
        );
        assert_eq!(
            is_assignable(&r_id, &w_id, &reg, &AssignabilityConfig::default()),
            Assignable::Yes,
            "and the other direction, which is the suite's struct_key_enum_1/_2 pair"
        );
    }

    /// The two halves of the rule, each on its own, and the `@final`
    /// exception where containment *is* the rule.
    #[test]
    fn enum_literals_that_disagree_are_not_assignable() {
        // Same value, different name.
        let mut reg = TypeRegistry::new();
        let (w, r) = pair(
            &mut reg,
            MinimalTypeObject::Enumerated(
                TypeObjectBuilder::enum_type("::E")
                    .bit_bound(32)
                    .literal("A", 1)
                    .build_minimal(),
            ),
            MinimalTypeObject::Enumerated(
                TypeObjectBuilder::enum_type("::E")
                    .bit_bound(32)
                    .literal("B", 1)
                    .build_minimal(),
            ),
        );
        assert!(!is_assignable(&w, &r, &reg, &AssignabilityConfig::default()).is_yes());

        // Same name, different value.
        let mut reg = TypeRegistry::new();
        let (w, r) = pair(
            &mut reg,
            MinimalTypeObject::Enumerated(
                TypeObjectBuilder::enum_type("::E")
                    .bit_bound(32)
                    .literal("A", 1)
                    .build_minimal(),
            ),
            MinimalTypeObject::Enumerated(
                TypeObjectBuilder::enum_type("::E")
                    .bit_bound(32)
                    .literal("A", 2)
                    .build_minimal(),
            ),
        );
        assert!(!is_assignable(&w, &r, &reg, &AssignabilityConfig::default()).is_yes());
    }

    /// A `@final` enumeration admits no extra literal on either side.
    #[test]
    fn a_final_enum_has_the_same_literals_on_both_sides() {
        use crate::type_object::flags::{EnumTypeFlag, StructTypeFlag};
        let build = |extra: bool| {
            let mut e = TypeObjectBuilder::enum_type("::E")
                .bit_bound(32)
                .literal("A", 1);
            if extra {
                e = e.literal("B", 2);
            }
            let mut e = e.build_minimal();
            e.enum_flags = EnumTypeFlag(StructTypeFlag::IS_FINAL);
            MinimalTypeObject::Enumerated(e)
        };
        let mut reg = TypeRegistry::new();
        let (w, r) = pair(&mut reg, build(true), build(false));
        assert!(
            !is_assignable(&w, &r, &reg, &AssignabilityConfig::default()).is_yes(),
            "a final enum's literal set is fixed"
        );
        let mut reg = TypeRegistry::new();
        let (w, r) = pair(&mut reg, build(true), build(true));
        assert_eq!(
            is_assignable(&w, &r, &reg, &AssignabilityConfig::default()),
            Assignable::Yes
        );
    }

    #[test]
    fn enum_bit_bound_mismatch_is_no() {
        let mut reg = reg();
        let w = MinimalTypeObject::Enumerated(
            TypeObjectBuilder::enum_type("::E")
                .bit_bound(32)
                .literal("A", 1)
                .build_minimal(),
        );
        let r = MinimalTypeObject::Enumerated(
            TypeObjectBuilder::enum_type("::E")
                .bit_bound(16)
                .literal("A", 1)
                .build_minimal(),
        );
        let wh = crate::hash::compute_minimal_hash(&w).unwrap();
        let rh = crate::hash::compute_minimal_hash(&r).unwrap();
        reg.insert_minimal(wh, w);
        reg.insert_minimal(rh, r);

        let a = is_assignable(
            &TypeIdentifier::EquivalenceHashMinimal(wh),
            &TypeIdentifier::EquivalenceHashMinimal(rh),
            &reg,
            &AssignabilityConfig::default(),
        );
        assert!(!a.is_yes());
    }

    #[test]
    fn enum_identical_labels_is_yes() {
        let mut reg = reg();
        let w = MinimalTypeObject::Enumerated(
            TypeObjectBuilder::enum_type("::E")
                .bit_bound(32)
                .literal("A", 1)
                .literal("B", 2)
                .build_minimal(),
        );
        let r = MinimalTypeObject::Enumerated(
            TypeObjectBuilder::enum_type("::E")
                .bit_bound(32)
                .literal("A", 1)
                .literal("B", 2)
                .literal("C", 3) // reader knows an extra label, writer labels are a subset
                .build_minimal(),
        );
        let wh = crate::hash::compute_minimal_hash(&w).unwrap();
        let rh = crate::hash::compute_minimal_hash(&r).unwrap();
        reg.insert_minimal(wh, w);
        reg.insert_minimal(rh, r);

        let a = is_assignable(
            &TypeIdentifier::EquivalenceHashMinimal(wh),
            &TypeIdentifier::EquivalenceHashMinimal(rh),
            &reg,
            &AssignabilityConfig::default(),
        );
        assert!(a.is_yes());
    }

    // ---- §7.2.2.4.5 Inheritance flatten + edge-cases ----

    #[test]
    fn flatten_inheritance_no_base_returns_struct_unchanged() {
        let s = TypeObjectBuilder::struct_type("::S")
            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
            .build_minimal();
        let reg = reg();
        let flat = flatten_inheritance(&s, &reg, 8).unwrap();
        assert_eq!(flat.member_seq.len(), 1);
    }

    #[test]
    fn flatten_inheritance_two_levels_concatenates_base_first() {
        // Root → Mid → Derived. Result member order: [Root, Mid, Derived].
        let mut reg = reg();
        let root = TypeObjectBuilder::struct_type("::Root")
            .member("r", TypeIdentifier::Primitive(PrimitiveKind::Int8), |m| {
                m.id(101)
            })
            .build_minimal();
        let root_h = compute_minimal_hash(&MinimalTypeObject::Struct(root.clone())).unwrap();
        reg.insert_minimal(root_h, MinimalTypeObject::Struct(root));

        let mid = TypeObjectBuilder::struct_type("::Mid")
            .base(TypeIdentifier::EquivalenceHashMinimal(root_h))
            .member("m", TypeIdentifier::Primitive(PrimitiveKind::Int16), |m| {
                m.id(202)
            })
            .build_minimal();
        let mid_h = compute_minimal_hash(&MinimalTypeObject::Struct(mid.clone())).unwrap();
        reg.insert_minimal(mid_h, MinimalTypeObject::Struct(mid));

        let derived = TypeObjectBuilder::struct_type("::Derived")
            .base(TypeIdentifier::EquivalenceHashMinimal(mid_h))
            .member("d", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.id(303)
            })
            .build_minimal();

        let flat = flatten_inheritance(&derived, &reg, 8).unwrap();
        // 3 members, base_type gone.
        assert_eq!(flat.header.base_type, TypeIdentifier::None);
        assert_eq!(flat.member_seq.len(), 3);
        // First member is 'r' (root); last is 'd' (derived).
        let first_id = flat.member_seq[0].common.member_id;
        let last_id = flat.member_seq[2].common.member_id;
        assert_ne!(first_id, last_id);
    }

    #[test]
    fn inheritance_conflict_same_id() {
        let mut reg = reg();
        let base = TypeObjectBuilder::struct_type("::B")
            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.id(7)
            })
            .build_minimal();
        let bh = compute_minimal_hash(&MinimalTypeObject::Struct(base.clone())).unwrap();
        reg.insert_minimal(bh, MinimalTypeObject::Struct(base));

        let derived = TypeObjectBuilder::struct_type("::D")
            .base(TypeIdentifier::EquivalenceHashMinimal(bh))
            .member("c", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.id(7) // same ID as base member 'a' → conflict
            })
            .build_minimal();
        let err = flatten_inheritance(&derived, &reg, 8).unwrap_err();
        assert!(matches!(
            err,
            InheritanceError::InheritanceConflict { reason, .. }
                if reason.contains("member_id")
        ));
    }

    #[test]
    fn inheritance_conflict_same_name() {
        let mut reg = reg();
        let base = TypeObjectBuilder::struct_type("::B")
            .member(
                "dup",
                TypeIdentifier::Primitive(PrimitiveKind::Int32),
                |m| m.id(1),
            )
            .build_minimal();
        let bh = compute_minimal_hash(&MinimalTypeObject::Struct(base.clone())).unwrap();
        reg.insert_minimal(bh, MinimalTypeObject::Struct(base));

        let derived = TypeObjectBuilder::struct_type("::D")
            .base(TypeIdentifier::EquivalenceHashMinimal(bh))
            .member(
                "dup",
                TypeIdentifier::Primitive(PrimitiveKind::Int32),
                |m| {
                    m.id(2) // different ID, but same name → conflict
                },
            )
            .build_minimal();
        let err = flatten_inheritance(&derived, &reg, 8).unwrap_err();
        assert!(matches!(
            err,
            InheritanceError::InheritanceConflict { reason, .. }
                if reason.contains("name_hash")
        ));
    }

    #[test]
    fn flat_type_construction_two_levels() {
        // §7.2.2.4.5 — the hypothetical flat representation of a
        // 2-level inheritance construct has members in base-derived
        // order.
        let mut reg = reg();
        let base = TypeObjectBuilder::struct_type("::Base")
            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.id(1)
            })
            .member("b", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.id(2)
            })
            .build_minimal();
        let bh = compute_minimal_hash(&MinimalTypeObject::Struct(base.clone())).unwrap();
        reg.insert_minimal(bh, MinimalTypeObject::Struct(base));

        let derived = TypeObjectBuilder::struct_type("::Derived")
            .base(TypeIdentifier::EquivalenceHashMinimal(bh))
            .member("c", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.id(3)
            })
            .build_minimal();

        let flat = flatten_inheritance(&derived, &reg, 8).unwrap();
        assert_eq!(flat.member_seq.len(), 3);
        assert_eq!(flat.member_seq[0].common.member_id, 1);
        assert_eq!(flat.member_seq[1].common.member_id, 2);
        assert_eq!(flat.member_seq[2].common.member_id, 3);
    }

    #[test]
    fn two_level_inheritance_assignability_chain() {
        // Writer and reader each have 2-level inheritance, both
        // produce the same flat member sequence → assignable.
        let mut reg = reg();

        // ---- Writer side ----
        let w_root = TypeObjectBuilder::struct_type("::WRoot")
            .member("r", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.id(1)
            })
            .build_minimal();
        let wr_h = compute_minimal_hash(&MinimalTypeObject::Struct(w_root.clone())).unwrap();
        reg.insert_minimal(wr_h, MinimalTypeObject::Struct(w_root));

        let w_mid = TypeObjectBuilder::struct_type("::WMid")
            .base(TypeIdentifier::EquivalenceHashMinimal(wr_h))
            .member("m", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.id(2)
            })
            .build_minimal();

        let w_flat = flatten_inheritance(&w_mid, &reg, 8).unwrap();
        assert_eq!(w_flat.member_seq.len(), 2);

        // ---- Reader side (same structure, different type names) ----
        let r_root = TypeObjectBuilder::struct_type("::RRoot")
            .member("r", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.id(1)
            })
            .build_minimal();
        let rr_h = compute_minimal_hash(&MinimalTypeObject::Struct(r_root.clone())).unwrap();
        reg.insert_minimal(rr_h, MinimalTypeObject::Struct(r_root));

        let r_mid = TypeObjectBuilder::struct_type("::RMid")
            .base(TypeIdentifier::EquivalenceHashMinimal(rr_h))
            .member("m", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.id(2)
            })
            .build_minimal();

        let r_flat = flatten_inheritance(&r_mid, &reg, 8).unwrap();
        assert_eq!(r_flat.member_seq.len(), 2);

        // Direct member-by-member compare via assignability.
        let w_to = MinimalTypeObject::Struct(w_flat.clone());
        let r_to = MinimalTypeObject::Struct(r_flat.clone());
        let wh = compute_minimal_hash(&w_to).unwrap();
        let rh = compute_minimal_hash(&r_to).unwrap();
        reg.insert_minimal(wh, w_to);
        reg.insert_minimal(rh, r_to);
        let a = is_assignable(
            &TypeIdentifier::EquivalenceHashMinimal(wh),
            &TypeIdentifier::EquivalenceHashMinimal(rh),
            &reg,
            &AssignabilityConfig::default(),
        );
        assert!(a.is_yes(), "got {a:?}");
    }

    #[test]
    fn enum_not_assignable_strict_default() {
        // Same value, but different names → the strict default
        // (no `@ignore_literal_names` annotation, no config flag)
        // must return assignable=No.
        let mut reg = reg();
        let w = MinimalTypeObject::Enumerated(
            TypeObjectBuilder::enum_type("::E")
                .bit_bound(32)
                .literal("RED", 1)
                .build_minimal(),
        );
        let r = MinimalTypeObject::Enumerated(
            TypeObjectBuilder::enum_type("::E")
                .bit_bound(32)
                .literal("ROUGE", 1) // same value, different name
                .build_minimal(),
        );
        let wh = crate::hash::compute_minimal_hash(&w).unwrap();
        let rh = crate::hash::compute_minimal_hash(&r).unwrap();
        reg.insert_minimal(wh, w);
        reg.insert_minimal(rh, r);

        let a = is_assignable(
            &TypeIdentifier::EquivalenceHashMinimal(wh),
            &TypeIdentifier::EquivalenceHashMinimal(rh),
            &reg,
            &AssignabilityConfig::default(),
        );
        assert!(!a.is_yes());
    }

    #[test]
    fn enum_assignable_with_ignore_literal_names() {
        // Same value, different names — but config or a flag sets
        // ignore_literal_names; the result must be Yes.
        let mut reg = reg();
        let w = MinimalTypeObject::Enumerated(
            TypeObjectBuilder::enum_type("::E")
                .bit_bound(32)
                .literal("RED", 1)
                .literal("GREEN", 2)
                .build_minimal(),
        );
        let r = MinimalTypeObject::Enumerated(
            TypeObjectBuilder::enum_type("::E")
                .bit_bound(32)
                .literal("ROUGE", 1)
                .literal("VERT", 2)
                .build_minimal(),
        );
        let wh = crate::hash::compute_minimal_hash(&w).unwrap();
        let rh = crate::hash::compute_minimal_hash(&r).unwrap();
        reg.insert_minimal(wh, w);
        reg.insert_minimal(rh, r);

        let cfg = AssignabilityConfig {
            ignore_literal_names: true,
            ..AssignabilityConfig::default()
        };
        let a = is_assignable(
            &TypeIdentifier::EquivalenceHashMinimal(wh),
            &TypeIdentifier::EquivalenceHashMinimal(rh),
            &reg,
            &cfg,
        );
        assert!(a.is_yes());
    }

    /// §7.3.1.2.1.11: "If the `@ignore_literal_names` annotation has
    /// the value TRUE in **both** enumerated types, then the literal
    /// names are ignored ... Otherwise the literal names are
    /// considered."
    ///
    /// One side alone does not decide it. These two cases used to assert
    /// the opposite — "when the writer sets
    /// `EnumTypeFlag::IGNORE_LITERAL_NAMES`, it acts for the comparison
    /// just like the config flag" — which let a writer annotate its own
    /// enum and be assignable to a reader that had said nothing.
    #[test]
    fn one_sided_ignore_literal_names_does_not_drop_the_name_rule() {
        for (on_writer, on_reader) in [(true, false), (false, true)] {
            let mut reg = reg();
            let flag = crate::type_object::flags::EnumTypeFlag(
                crate::type_object::flags::EnumTypeFlag::IGNORE_LITERAL_NAMES,
            );
            let mut w_e = TypeObjectBuilder::enum_type("::E")
                .bit_bound(32)
                .literal("RED", 1)
                .build_minimal();
            if on_writer {
                w_e.enum_flags = flag;
            }
            let mut r_e = TypeObjectBuilder::enum_type("::E")
                .bit_bound(32)
                .literal("ROUGE", 1)
                .build_minimal();
            if on_reader {
                r_e.enum_flags = flag;
            }
            let w = MinimalTypeObject::Enumerated(w_e);
            let r = MinimalTypeObject::Enumerated(r_e);
            let wh = crate::hash::compute_minimal_hash(&w).unwrap();
            let rh = crate::hash::compute_minimal_hash(&r).unwrap();
            reg.insert_minimal(wh, w);
            reg.insert_minimal(rh, r);

            let a = is_assignable(
                &TypeIdentifier::EquivalenceHashMinimal(wh),
                &TypeIdentifier::EquivalenceHashMinimal(rh),
                &reg,
                &AssignabilityConfig::default(),
            );
            assert!(
                !a.is_yes(),
                "on_writer={on_writer} on_reader={on_reader}: {a:?}"
            );
        }
    }

    /// The clause's own example: both types carry the annotation, the
    /// literal names differ, and the two are assignable.
    #[test]
    fn ignore_literal_names_on_both_sides_drops_the_name_rule() {
        let mut reg = reg();
        let flag = crate::type_object::flags::EnumTypeFlag(
            crate::type_object::flags::EnumTypeFlag::IGNORE_LITERAL_NAMES,
        );
        let mut w_e = TypeObjectBuilder::enum_type("::E")
            .bit_bound(32)
            .literal("RED", 1)
            .build_minimal();
        w_e.enum_flags = flag;
        let mut r_e = TypeObjectBuilder::enum_type("::E")
            .bit_bound(32)
            .literal("ROUGE", 1)
            .build_minimal();
        r_e.enum_flags = flag;
        let w = MinimalTypeObject::Enumerated(w_e);
        let r = MinimalTypeObject::Enumerated(r_e);
        let wh = crate::hash::compute_minimal_hash(&w).unwrap();
        let rh = crate::hash::compute_minimal_hash(&r).unwrap();
        reg.insert_minimal(wh, w);
        reg.insert_minimal(rh, r);

        let a = is_assignable(
            &TypeIdentifier::EquivalenceHashMinimal(wh),
            &TypeIdentifier::EquivalenceHashMinimal(rh),
            &reg,
            &AssignabilityConfig::default(),
        );
        assert!(a.is_yes(), "{a:?}");
    }

    #[test]
    fn struct_vs_enum_type_object_kinds_dont_match() {
        let mut reg = reg();
        let w = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Appendable)
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .build_minimal(),
        );
        let r = MinimalTypeObject::Enumerated(
            TypeObjectBuilder::enum_type("::X")
                .bit_bound(32)
                .literal("A", 1)
                .build_minimal(),
        );
        let wh = crate::hash::compute_minimal_hash(&w).unwrap();
        let rh = crate::hash::compute_minimal_hash(&r).unwrap();
        reg.insert_minimal(wh, w);
        reg.insert_minimal(rh, r);

        let a = is_assignable(
            &TypeIdentifier::EquivalenceHashMinimal(wh),
            &TypeIdentifier::EquivalenceHashMinimal(rh),
            &reg,
            &AssignabilityConfig::default(),
        );
        assert!(!a.is_yes());
    }

    #[test]
    fn appendable_struct_reader_longer_than_writer_is_yes() {
        // This test read `..._is_no` and asserted the opposite until the
        // corpus contradicted it. §7.2.4.4.4.4 compares the shorter
        // member sequence as a prefix of the longer, either way round,
        // and `evolution-appendable-tail` is exactly this shape with the
        // reader reading the writer's payload in every encoding and both
        // byte orders. A test that asserts what the code does rather
        // than what the clause says is a test that cannot find this.
        let mut reg = reg();
        let writer = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Appendable)
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .build_minimal(),
        );
        let reader = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Appendable)
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .member("b", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .build_minimal(),
        );
        let wh = crate::hash::compute_minimal_hash(&writer).unwrap();
        let rh = crate::hash::compute_minimal_hash(&reader).unwrap();
        reg.insert_minimal(wh, writer);
        reg.insert_minimal(rh, reader);

        assert_eq!(
            is_assignable(
                &TypeIdentifier::EquivalenceHashMinimal(wh),
                &TypeIdentifier::EquivalenceHashMinimal(rh),
                &reg,
                &AssignabilityConfig::default(),
            ),
            Assignable::Yes,
            "an appendable reader that grew a tail member reads an older writer's sample"
        );
    }

    #[test]
    fn an_appendable_tail_member_that_must_be_understood_is_no() {
        // The one thing that does stop an appendable pair past the
        // prefix. Without it the rule above would let a reader declare a
        // member it must understand and then never be sent one.
        let mut reg = reg();
        let writer = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Appendable)
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .build_minimal(),
        );
        let reader = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Appendable)
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .member("b", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                    m.must_understand()
                })
                .build_minimal(),
        );
        let wh = crate::hash::compute_minimal_hash(&writer).unwrap();
        let rh = crate::hash::compute_minimal_hash(&reader).unwrap();
        reg.insert_minimal(wh, writer);
        reg.insert_minimal(rh, reader);

        assert!(
            !is_assignable(
                &TypeIdentifier::EquivalenceHashMinimal(wh),
                &TypeIdentifier::EquivalenceHashMinimal(rh),
                &reg,
                &AssignabilityConfig::default(),
            )
            .is_yes()
        );
    }

    #[test]
    fn a_mutable_reader_member_the_writer_lacks_is_yes_unless_it_must_be_understood() {
        // This test read `mutable_reader_member_missing_in_writer_non_
        // optional_is_no` and turned on `@optional`, which is not what
        // §7.2.4.4.4.4 turns on. `@optional` says a member may be absent
        // from a *sample*; `must_understand` says whether it may be
        // absent from the *type*. Eight corpus pairs are the ordinary
        // case and all eight read successfully.
        let mut reg = reg();
        let writer = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Mutable)
                .member("b", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                    m.id(2)
                })
                .build_minimal(),
        );
        let reader = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Mutable)
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                    m.id(1) // neither optional nor must-understand
                })
                .member("b", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                    m.id(2)
                })
                .build_minimal(),
        );
        let wh = crate::hash::compute_minimal_hash(&writer).unwrap();
        let rh = crate::hash::compute_minimal_hash(&reader).unwrap();
        reg.insert_minimal(wh, writer);
        reg.insert_minimal(rh, reader);
        assert_eq!(
            is_assignable(
                &TypeIdentifier::EquivalenceHashMinimal(wh),
                &TypeIdentifier::EquivalenceHashMinimal(rh),
                &reg,
                &AssignabilityConfig::default(),
            ),
            Assignable::Yes
        );

        // The same pair with the added member marked, which is the
        // annotation the clause reads.
        let mut marked = TypeRegistry::new();
        let writer = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Mutable)
                .member("b", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                    m.id(2)
                })
                .build_minimal(),
        );
        let reader = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Mutable)
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                    m.id(1).must_understand()
                })
                .member("b", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                    m.id(2)
                })
                .build_minimal(),
        );
        let wh = crate::hash::compute_minimal_hash(&writer).unwrap();
        let rh = crate::hash::compute_minimal_hash(&reader).unwrap();
        marked.insert_minimal(wh, writer);
        marked.insert_minimal(rh, reader);
        assert!(
            !is_assignable(
                &TypeIdentifier::EquivalenceHashMinimal(wh),
                &TypeIdentifier::EquivalenceHashMinimal(rh),
                &marked,
                &AssignabilityConfig::default(),
            )
            .is_yes()
        );
    }

    #[test]
    fn a_mutable_writer_member_the_reader_lacks_and_must_understand_is_no() {
        // The direction the `cdr` version did not check at all: a writer
        // that marks a member `@must_understand` and a reader that does
        // not declare it were called assignable, and §7.2.4.4.4.4 makes
        // them not. The silent half of the same rule.
        let mut reg = reg();
        let writer = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Mutable)
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                    m.id(1).must_understand()
                })
                .member("b", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                    m.id(2)
                })
                .build_minimal(),
        );
        let reader = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Mutable)
                .member("b", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                    m.id(2)
                })
                .build_minimal(),
        );
        let wh = crate::hash::compute_minimal_hash(&writer).unwrap();
        let rh = crate::hash::compute_minimal_hash(&reader).unwrap();
        reg.insert_minimal(wh, writer);
        reg.insert_minimal(rh, reader);
        assert!(
            !is_assignable(
                &TypeIdentifier::EquivalenceHashMinimal(wh),
                &TypeIdentifier::EquivalenceHashMinimal(rh),
                &reg,
                &AssignabilityConfig::default(),
            )
            .is_yes()
        );
    }

    #[test]
    fn final_struct_member_count_mismatch_is_no() {
        let mut reg = reg();
        let writer = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Final)
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .member("b", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .build_minimal(),
        );
        let reader = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Final)
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .build_minimal(),
        );
        let wh = crate::hash::compute_minimal_hash(&writer).unwrap();
        let rh = crate::hash::compute_minimal_hash(&reader).unwrap();
        reg.insert_minimal(wh, writer);
        reg.insert_minimal(rh, reader);

        assert!(
            !is_assignable(
                &TypeIdentifier::EquivalenceHashMinimal(wh),
                &TypeIdentifier::EquivalenceHashMinimal(rh),
                &reg,
                &AssignabilityConfig::default(),
            )
            .is_yes()
        );
    }

    /// The three widenings the removed coercion matrix used to accept —
    /// signed, unsigned-into-signed and float — are each rejected. The
    /// suite's `struct_uint16_int8` states the same rule in its own
    /// description: "Primitive types must match exactly for
    /// assignability."
    #[test]
    fn primitive_widening_is_rejected_per_table_15() {
        assert!(!primitive_compatible(PrimitiveKind::Int16, PrimitiveKind::Int64).is_yes());
        assert!(!primitive_compatible(PrimitiveKind::Byte, PrimitiveKind::Int32).is_yes());
        assert!(!primitive_compatible(PrimitiveKind::UInt16, PrimitiveKind::Int32).is_yes());
        assert!(!primitive_compatible(PrimitiveKind::Float32, PrimitiveKind::Float64).is_yes());
    }

    #[test]
    fn primitive_narrowing_and_cross_family_are_rejected() {
        assert!(!primitive_compatible(PrimitiveKind::Float64, PrimitiveKind::Int32).is_yes());
        assert!(!primitive_compatible(PrimitiveKind::Int64, PrimitiveKind::Int32).is_yes());
        assert!(!primitive_compatible(PrimitiveKind::UInt8, PrimitiveKind::Int8).is_yes());
    }

    /// The single row Table 15 does have, checked over every
    /// `PrimitiveKind`: each is assignable from itself and from no other.
    #[test]
    fn only_the_identical_primitive_kind_is_assignable() {
        use PrimitiveKind::*;
        let all = [
            Boolean, Byte, Int8, Int16, Int32, Int64, UInt8, UInt16, UInt32, UInt64, Float32,
            Float64, Float128, Char8, Char16,
        ];
        for w in all {
            for r in all {
                assert_eq!(
                    primitive_compatible(w, r).is_yes(),
                    w == r,
                    "Table 15 accepts {w:?} → {r:?} only when the kinds are identical"
                );
            }
        }
    }

    #[test]
    fn assignable_is_yes_matches_expectation() {
        assert!(Assignable::Yes.is_yes());
        assert!(!Assignable::No("reason").is_yes());
    }

    // Bug QT (#76): a typed endpoint whose generated type carries a *complete*
    // TypeIdentifier (EquivalenceHashComplete) whose TypeObject is absent from
    // the registry must still match itself. Before the fix, `is_assignable`
    // resolved the alias chain FIRST and returned `Unknown` for the absent
    // hash, BEFORE the identity short-circuit — so a writer + reader of the
    // SAME complete type failed to match.
    #[test]
    fn equal_complete_hash_is_assignable_with_empty_registry() {
        let reg = TypeRegistry::new();
        let h = crate::type_identifier::EquivalenceHash([0xCE; 14]);
        let ti = TypeIdentifier::EquivalenceHashComplete(h);
        assert!(
            is_assignable(&ti, &ti, &reg, &AssignabilityConfig::default()).is_yes(),
            "identical complete TypeIdentifiers must be assignable even with an empty registry"
        );
    }

    #[test]
    fn equal_minimal_hash_is_assignable_with_empty_registry() {
        let reg = TypeRegistry::new();
        let h = crate::type_identifier::EquivalenceHash([0x4D; 14]);
        let ti = TypeIdentifier::EquivalenceHashMinimal(h);
        assert!(is_assignable(&ti, &ti, &reg, &AssignabilityConfig::default()).is_yes());
    }

    /// Two endpoints announcing *different* complete `TypeIdentifier`s
    /// whose `CompleteTypeObject`s the registry holds are compared
    /// structurally — and a non-assignable pair is refused.
    ///
    /// The lookup used to be `registry.get_minimal(complete_hash)`, and a
    /// `CompleteTypeObject` is filed under the hash of the complete
    /// object while its minimal projection is filed under the hash of the
    /// minimal one. The two hashes differ by construction (XTypes 1.3
    /// §7.3.4.5 derives both forms from one model and hashes each), so
    /// that lookup missed every time — even for a type the caller had
    /// registered itself — and the pair came back "unknown", which the
    /// DCPS runtime reads as "not resolved yet" and matches
    /// optimistically. The OMG `dds-xtypes` suite is 335 cases of exactly
    /// this comparison and every incompatible pair in it matched.
    #[test]
    fn registered_complete_objects_decide_a_differing_hash_pair() {
        let i32_id = TypeIdentifier::Primitive(crate::type_identifier::PrimitiveKind::Int32);
        let mut reg = TypeRegistry::new();
        let mut register = |name: &str, ext, members: &[&str]| {
            let mut b = TypeObjectBuilder::struct_type(name).extensibility(ext);
            for m in members {
                b = b.member(*m, i32_id.clone(), |m| m);
            }
            let c = crate::type_object::complete::CompleteTypeObject::Struct(b.build_complete());
            let hash = crate::hash::compute_complete_hash(&c).expect("hash");
            reg.insert_complete(hash, c);
            TypeIdentifier::EquivalenceHashComplete(hash)
        };
        let one = register("::xt::One", Extensibility::Final, &["x1"]);
        let two = register("::xt::Two", Extensibility::Final, &["x1", "x2"]);
        let one_again = register("::xt::OneAgain", Extensibility::Final, &["x1"]);

        let cfg = AssignabilityConfig {
            ignore_member_names: true,
            ..AssignabilityConfig::default()
        };
        assert!(
            !is_assignable(&one, &two, &reg, &cfg).is_yes(),
            "§7.2.4.4.4.4: a @final struct is not assignable from one with a \
             different member count"
        );
        assert!(
            is_assignable(&one, &one_again, &reg, &cfg).is_yes(),
            "the same member sequence under two names is assignable when \
             member names are ignored"
        );
    }

    #[test]
    fn differing_complete_hashes_unknown_objects_are_not_assignable() {
        let reg = TypeRegistry::new();
        let wh = crate::type_identifier::EquivalenceHash([0x01; 14]);
        let rh = crate::type_identifier::EquivalenceHash([0x02; 14]);
        assert!(
            !is_assignable(
                &TypeIdentifier::EquivalenceHashComplete(wh),
                &TypeIdentifier::EquivalenceHashComplete(rh),
                &reg,
                &AssignabilityConfig::default(),
            )
            .is_yes()
        );
    }

    /// A bitmask of a given bit bound is assignable to and from exactly
    /// one unsigned integer — the one Table 15 pairs with its range.
    ///
    /// XTypes 1.3 §7.2.4.4.4, Table 15, rows two to five. The pair is
    /// checked in both directions because the row is symmetric and the
    /// two directions reach two different match arms of `check_direct`.
    /// The ranges are read off OpenDDS
    /// `dds/DCPS/XTypes/TypeAssignability.cpp` (`assignable_bitmask`,
    /// `assignable_primitive`), which carries the same four.
    #[test]
    fn bitmask_is_assignable_to_the_uint_of_its_bound() {
        for (bit_bound, holder) in [
            (1_u16, PrimitiveKind::UInt8),
            (8, PrimitiveKind::UInt8),
            (9, PrimitiveKind::UInt16),
            (16, PrimitiveKind::UInt16),
            (17, PrimitiveKind::UInt32),
            (32, PrimitiveKind::UInt32),
            (33, PrimitiveKind::UInt64),
            (64, PrimitiveKind::UInt64),
        ] {
            let (reg, mask) = bitmask_registry(bit_bound);
            let uint = TypeIdentifier::Primitive(holder);
            let cfg = AssignabilityConfig::default();
            assert!(
                is_assignable(&mask, &uint, &reg, &cfg).is_yes(),
                "bitmask({bit_bound}) → {holder:?}"
            );
            assert!(
                is_assignable(&uint, &mask, &reg, &cfg).is_yes(),
                "{holder:?} → bitmask({bit_bound})"
            );
        }
    }

    /// The four ranges do not nest. A bitmask of 16 bits fits into a
    /// `uint32` arithmetically and is still not assignable to it:
    /// Table 15 pairs it with `uint16` and with nothing else, because a
    /// reader must not have to parse the sample differently depending
    /// on which writer sent it (§7.2.4.4.4).
    #[test]
    fn bitmask_is_not_assignable_to_a_wider_uint() {
        let (reg, mask) = bitmask_registry(16);
        let cfg = AssignabilityConfig::default();
        for other in [
            PrimitiveKind::UInt8,
            PrimitiveKind::UInt32,
            PrimitiveKind::UInt64,
            PrimitiveKind::Int16,
            PrimitiveKind::Byte,
        ] {
            let ti = TypeIdentifier::Primitive(other);
            assert!(
                !is_assignable(&mask, &ti, &reg, &cfg).is_yes(),
                "bitmask(16) → {other:?} is not a row of Table 15"
            );
            assert!(
                !is_assignable(&ti, &mask, &reg, &cfg).is_yes(),
                "{other:?} → bitmask(16) is not a row of Table 15"
            );
        }
    }

    /// The arms are reached by *bitmask*, not by "hash on one side".
    /// A struct against a primitive stays what it was — no row.
    #[test]
    fn a_struct_against_a_uint_is_still_no_row() {
        let mut reg = TypeRegistry::new();
        let obj = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::S")
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::UInt32), |m| m)
                .build_minimal(),
        );
        let h = compute_minimal_hash(&obj).unwrap();
        reg.insert_minimal(h, obj);
        let cfg = AssignabilityConfig::default();
        let s = TypeIdentifier::EquivalenceHashMinimal(h);
        let u = TypeIdentifier::Primitive(PrimitiveKind::UInt32);
        assert!(!is_assignable(&s, &u, &reg, &cfg).is_yes());
        assert!(!is_assignable(&u, &s, &reg, &cfg).is_yes());
    }

    /// A registry that holds one bitmask of `bit_bound` bits, and the
    /// identifier that names it.
    /// XTypes 1.3 §7.2.4.4.4.2, the `STRING_TYPE` row, with `0` read as
    /// the unbounded length §7.3.4.5 writes it as. `string` → `string<10>`
    /// must fail the bound check that `string<20>` → `string<10>` fails;
    /// reading the writer's `0` arithmetically let it through.
    /// The suite's `string_string10_check` / `seq(int32)_seq(int32,10)_check_bounds`.
    #[test]
    fn an_unbounded_writer_bound_exceeds_every_reader_bound() {
        let strict = AssignabilityConfig {
            ignore_string_bounds: false,
            ignore_sequence_bounds: false,
            ..AssignabilityConfig::default()
        };
        let unbounded = TypeIdentifier::String8Small { bound: 0 };
        let bounded = TypeIdentifier::String8Small { bound: 10 };
        assert!(
            !is_assignable(&unbounded, &bounded, &reg(), &strict).is_yes(),
            "an unbounded writer string can send more than string<10> holds"
        );
        assert!(
            is_assignable(&bounded, &unbounded, &reg(), &strict).is_yes(),
            "an unbounded reader holds every writer bound"
        );
        assert!(
            is_assignable(&unbounded, &unbounded, &reg(), &strict).is_yes(),
            "two unbounded strings are the same type"
        );

        let w_seq = TypeIdentifier::PlainSequenceSmall {
            header: crate::type_identifier::PlainCollectionHeader::default(),
            bound: 0,
            element: alloc::boxed::Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
        };
        let r_seq = TypeIdentifier::PlainSequenceSmall {
            header: crate::type_identifier::PlainCollectionHeader::default(),
            bound: 10,
            element: alloc::boxed::Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
        };
        assert!(
            !is_assignable(&w_seq, &r_seq, &reg(), &strict).is_yes(),
            "an unbounded writer sequence can send more than sequence<int32,10> holds"
        );
    }

    /// §7.2.4.4.4.4: "Any members in T1 and T2 that have the same name
    /// also have the same ID and any members with the same ID also have
    /// the same name." The suite's `struct_1`/`struct_2`: `x1` at id 1
    /// against `x1` at id 2, with `x5` at id 5 on both sides so that the
    /// id walk finds a clean match and never looks at `x1`.
    #[test]
    fn a_member_name_at_two_different_ids_is_refused() {
        let mut reg = reg();
        let writer = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Mutable)
                .member("x1", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                    m.id(1)
                })
                .member("x5", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                    m.id(5)
                })
                .build_minimal(),
        );
        let reader = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Mutable)
                .member("x1", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                    m.id(2)
                })
                .member("x5", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                    m.id(5)
                })
                .build_minimal(),
        );
        let wh = crate::hash::compute_minimal_hash(&writer).unwrap();
        let rh = crate::hash::compute_minimal_hash(&reader).unwrap();
        reg.insert_minimal(wh, writer);
        reg.insert_minimal(rh, reader);
        let w = TypeIdentifier::EquivalenceHashMinimal(wh);
        let r = TypeIdentifier::EquivalenceHashMinimal(rh);

        assert!(
            !is_assignable(&w, &r, &reg, &AssignabilityConfig::default()).is_yes(),
            "one name at two ids is what the clause refuses"
        );
        let ignoring = AssignabilityConfig {
            ignore_member_names: true,
            ..AssignabilityConfig::default()
        };
        assert!(
            is_assignable(&w, &r, &reg, &ignoring).is_yes(),
            "ignore_member_names is the switch §7.6.3.4.1 gives for exactly this rule"
        );
    }

    /// A union member's id follows declaration order, so the same three
    /// cases written in a different order carry different ids for the
    /// same label. §7.2.4.4.4.5 asks about the *types* a shared label
    /// selects, not about the ids. The suite's `union_1`/`union_2`.
    #[test]
    fn a_shared_union_label_may_carry_different_member_ids() {
        let mut reg = reg();
        let i16t = TypeIdentifier::Primitive(PrimitiveKind::Int16);
        let i32t = TypeIdentifier::Primitive(PrimitiveKind::Int32);
        let writer = MinimalTypeObject::Union(
            TypeObjectBuilder::union_type("::U", TypeIdentifier::Primitive(PrimitiveKind::UInt32))
                .case("x1", i16t.clone(), alloc::vec![1])
                .case("x2", i32t.clone(), alloc::vec![2])
                .build_minimal(),
        );
        let reader = MinimalTypeObject::Union(
            TypeObjectBuilder::union_type("::U", TypeIdentifier::Primitive(PrimitiveKind::UInt32))
                .case("x2", i32t, alloc::vec![2])
                .case("x1", i16t, alloc::vec![1])
                .build_minimal(),
        );
        let wh = crate::hash::compute_minimal_hash(&writer).unwrap();
        let rh = crate::hash::compute_minimal_hash(&reader).unwrap();
        reg.insert_minimal(wh, writer);
        reg.insert_minimal(rh, reader);
        let w = TypeIdentifier::EquivalenceHashMinimal(wh);
        let r = TypeIdentifier::EquivalenceHashMinimal(rh);

        let ignoring = AssignabilityConfig {
            ignore_member_names: true,
            ..AssignabilityConfig::default()
        };
        assert_eq!(
            is_assignable(&w, &r, &reg, &ignoring),
            Assignable::Yes,
            "the same cases in a different order are the same union"
        );
        assert!(
            !is_assignable(&w, &r, &reg, &AssignabilityConfig::default()).is_yes(),
            "with names in play, x1 at id 0 against x1 at id 1 is the §7.2.4.4.4.5 conflict"
        );
    }

    /// `prevent_type_widening` (§7.6.3.4.1): the reader may not declare
    /// a member the writer does not. Without the flag the appendable
    /// tail is read as its default and the pair matches — the suite's
    /// `tc_prevent_type_widening_4` against `_5`.
    #[test]
    fn prevent_type_widening_refuses_a_reader_that_grew_a_member() {
        for ext in [Extensibility::Appendable, Extensibility::Mutable] {
            let mut reg = reg();
            let writer = MinimalTypeObject::Struct(
                TypeObjectBuilder::struct_type("::X")
                    .extensibility(ext)
                    .member("x1", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                    .build_minimal(),
            );
            let reader = MinimalTypeObject::Struct(
                TypeObjectBuilder::struct_type("::X")
                    .extensibility(ext)
                    .member("x1", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                    .member("x2", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                    .build_minimal(),
            );
            let wh = crate::hash::compute_minimal_hash(&writer).unwrap();
            let rh = crate::hash::compute_minimal_hash(&reader).unwrap();
            reg.insert_minimal(wh, writer);
            reg.insert_minimal(rh, reader);
            let w = TypeIdentifier::EquivalenceHashMinimal(wh);
            let r = TypeIdentifier::EquivalenceHashMinimal(rh);

            assert_eq!(
                is_assignable(&w, &r, &reg, &AssignabilityConfig::default()),
                Assignable::Yes,
                "{ext:?}: without the flag the reader's extra member takes its default"
            );
            let strict = AssignabilityConfig {
                prevent_type_widening: true,
                ..AssignabilityConfig::default()
            };
            assert!(
                !is_assignable(&w, &r, &reg, &strict).is_yes(),
                "{ext:?}: prevent_type_widening refuses exactly that member"
            );
        }
    }

    /// Registers two `MinimalTypeObject`s and hands back their hash
    /// identifiers, writer first.
    fn pair(
        reg: &mut TypeRegistry,
        w: MinimalTypeObject,
        r: MinimalTypeObject,
    ) -> (TypeIdentifier, TypeIdentifier) {
        let wh = crate::hash::compute_minimal_hash(&w).unwrap();
        let rh = crate::hash::compute_minimal_hash(&r).unwrap();
        reg.insert_minimal(wh, w);
        reg.insert_minimal(rh, r);
        (
            TypeIdentifier::EquivalenceHashMinimal(wh),
            TypeIdentifier::EquivalenceHashMinimal(rh),
        )
    }

    fn u32_disc() -> TypeIdentifier {
        TypeIdentifier::Primitive(PrimitiveKind::UInt32)
    }

    /// §7.2.4.4.4.5, the non-final rule: two unions need at least one
    /// case label in common. Without it every pair of unions passed —
    /// the per-label walk only ever asks about labels that *are*
    /// shared. The suite's `union_appendable_one_common_1` against
    /// `union_appendable_no_common_1`.
    #[test]
    fn two_unions_need_a_case_label_in_common() {
        let i32t = TypeIdentifier::Primitive(PrimitiveKind::Int32);
        for ext in [Extensibility::Appendable, Extensibility::Mutable] {
            let mut reg = TypeRegistry::new();
            let (w, r) = pair(
                &mut reg,
                MinimalTypeObject::Union(
                    TypeObjectBuilder::union_type("::U", u32_disc())
                        .extensibility(ext)
                        .case("x1", i32t.clone(), alloc::vec![1])
                        .build_minimal(),
                ),
                MinimalTypeObject::Union(
                    TypeObjectBuilder::union_type("::U", u32_disc())
                        .extensibility(ext)
                        .case("x11", i32t.clone(), alloc::vec![11])
                        .build_minimal(),
                ),
            );
            assert!(
                !is_assignable(&w, &r, &reg, &AssignabilityConfig::default()).is_yes(),
                "{ext:?}: no label in common is no agreement on any discriminator value"
            );

            let mut reg = TypeRegistry::new();
            let (w, r) = pair(
                &mut reg,
                MinimalTypeObject::Union(
                    TypeObjectBuilder::union_type("::U", u32_disc())
                        .extensibility(ext)
                        .case("x1", i32t.clone(), alloc::vec![1])
                        .build_minimal(),
                ),
                MinimalTypeObject::Union(
                    TypeObjectBuilder::union_type("::U", u32_disc())
                        .extensibility(ext)
                        .case("x1", i32t.clone(), alloc::vec![1])
                        .case("x11", i32t.clone(), alloc::vec![11])
                        .build_minimal(),
                ),
            );
            assert_eq!(
                is_assignable(&w, &r, &reg, &AssignabilityConfig::default()),
                Assignable::Yes,
                "{ext:?}: one shared label is what the clause asks for"
            );
        }
    }

    /// The `default:` case supplies no common label. Its label is not a
    /// discriminator value — it is whatever is left — so two unions
    /// whose only agreement is that each has a default agree on no
    /// value at all. The suite's `union_appendable_no_common_w_default`.
    #[test]
    fn a_default_case_is_not_a_case_label_in_common() {
        let u32t = TypeIdentifier::Primitive(PrimitiveKind::UInt32);
        let mut reg = TypeRegistry::new();
        let (w, r) = pair(
            &mut reg,
            MinimalTypeObject::Union(
                TypeObjectBuilder::union_type("::U", u32_disc())
                    .extensibility(Extensibility::Appendable)
                    .case("x5", u32t.clone(), alloc::vec![5])
                    .default_case("xd", u32t.clone())
                    .build_minimal(),
            ),
            MinimalTypeObject::Union(
                TypeObjectBuilder::union_type("::U", u32_disc())
                    .extensibility(Extensibility::Appendable)
                    .case("x15", u32t.clone(), alloc::vec![15])
                    .default_case("xd", u32t)
                    .build_minimal(),
            ),
        );
        assert!(
            !is_assignable(&w, &r, &reg, &AssignabilityConfig::default()).is_yes(),
            "two defaults are not a shared discriminator value"
        );
    }

    /// A `@final` union's member set is fixed, and a default member is a
    /// member. The suite runs both directions,
    /// `union_final_one_default_1` and `_2`.
    #[test]
    fn a_final_union_has_a_default_member_on_both_sides_or_neither() {
        let i32t = TypeIdentifier::Primitive(PrimitiveKind::Int32);
        let plain = || {
            MinimalTypeObject::Union(
                TypeObjectBuilder::union_type("::U", u32_disc())
                    .extensibility(Extensibility::Final)
                    .case(
                        "x1",
                        TypeIdentifier::Primitive(PrimitiveKind::Int32),
                        alloc::vec![1],
                    )
                    .build_minimal(),
            )
        };
        let with_default = || {
            MinimalTypeObject::Union(
                TypeObjectBuilder::union_type("::U", u32_disc())
                    .extensibility(Extensibility::Final)
                    .case(
                        "x1",
                        TypeIdentifier::Primitive(PrimitiveKind::Int32),
                        alloc::vec![1],
                    )
                    .default_case("xd", i32t.clone())
                    .build_minimal(),
            )
        };
        let mut reg = TypeRegistry::new();
        let (w, r) = pair(&mut reg, plain(), with_default());
        assert!(
            !is_assignable(&w, &r, &reg, &AssignabilityConfig::default()).is_yes(),
            "a final reader with a default the writer does not declare"
        );
        let mut reg = TypeRegistry::new();
        let (w, r) = pair(&mut reg, with_default(), plain());
        assert!(
            !is_assignable(&w, &r, &reg, &AssignabilityConfig::default()).is_yes(),
            "and the other direction"
        );
        let mut reg = TypeRegistry::new();
        let (w, r) = pair(&mut reg, plain(), plain());
        assert_eq!(
            is_assignable(&w, &r, &reg, &AssignabilityConfig::default()),
            Assignable::Yes,
        );
    }

    /// §7.2.4.4.4.5: a discriminator value that selects no explicit
    /// member of the other side selects that side's *default* member.
    /// Skipping the pair let a writer send `case 2: short` to a reader
    /// whose `default: long` is where value 2 lands. The suite's
    /// `union_int16_int32_default` against `union_int32_int32_default`,
    /// and `union_int32_default_int16` for the mirror.
    #[test]
    fn a_label_the_other_side_lacks_is_checked_against_its_default_member() {
        let i16t = TypeIdentifier::Primitive(PrimitiveKind::Int16);
        let i32t = TypeIdentifier::Primitive(PrimitiveKind::Int32);
        let explicit = |ty: TypeIdentifier| {
            MinimalTypeObject::Union(
                TypeObjectBuilder::union_type("::U", u32_disc())
                    .extensibility(Extensibility::Appendable)
                    .case(
                        "x1",
                        TypeIdentifier::Primitive(PrimitiveKind::Int16),
                        alloc::vec![1],
                    )
                    .case("x2", ty, alloc::vec![2])
                    .build_minimal(),
            )
        };
        let defaulted = |ty: TypeIdentifier| {
            MinimalTypeObject::Union(
                TypeObjectBuilder::union_type("::U", u32_disc())
                    .extensibility(Extensibility::Appendable)
                    .case(
                        "x1",
                        TypeIdentifier::Primitive(PrimitiveKind::Int16),
                        alloc::vec![1],
                    )
                    .default_case("x2", ty)
                    .build_minimal(),
            )
        };
        let ignoring = AssignabilityConfig {
            ignore_member_names: true,
            ..AssignabilityConfig::default()
        };

        let mut reg = TypeRegistry::new();
        let (w, r) = pair(&mut reg, explicit(i16t.clone()), defaulted(i32t.clone()));
        assert!(
            !is_assignable(&w, &r, &reg, &ignoring).is_yes(),
            "the writer's `case 2: int16` lands in the reader's `default: int32`"
        );
        let mut reg = TypeRegistry::new();
        let (w, r) = pair(&mut reg, explicit(i32t.clone()), defaulted(i32t.clone()));
        assert_eq!(
            is_assignable(&w, &r, &reg, &ignoring),
            Assignable::Yes,
            "the same shape with the widths agreeing"
        );
        let mut reg = TypeRegistry::new();
        let (w, r) = pair(&mut reg, defaulted(i32t.clone()), explicit(i16t));
        assert!(
            !is_assignable(&w, &r, &reg, &ignoring).is_yes(),
            "the mirror: the writer's default carries a value the reader reads as int16"
        );
        let mut reg = TypeRegistry::new();
        let (w, r) = pair(&mut reg, defaulted(i32t.clone()), explicit(i32t));
        assert_eq!(is_assignable(&w, &r, &reg, &ignoring), Assignable::Yes,);
    }

    /// §7.2.4.4.4.4: "There is at least one member m1 of T1 and one
    /// corresponding member m2 of T2 such that m1.id == m2.id." The
    /// suite's `struct_no_common_ids`.
    #[test]
    fn two_mutable_structs_need_a_member_id_in_common() {
        let i32t = TypeIdentifier::Primitive(PrimitiveKind::Int32);
        let mut reg = TypeRegistry::new();
        let (w, r) = pair(
            &mut reg,
            MinimalTypeObject::Struct(
                TypeObjectBuilder::struct_type("::X")
                    .extensibility(Extensibility::Mutable)
                    .member("x1", i32t.clone(), |m| m.id(1))
                    .build_minimal(),
            ),
            MinimalTypeObject::Struct(
                TypeObjectBuilder::struct_type("::X")
                    .extensibility(Extensibility::Mutable)
                    .member("x2", i32t.clone(), |m| m.id(2))
                    .build_minimal(),
            ),
        );
        assert!(
            !is_assignable(&w, &r, &reg, &AssignabilityConfig::default()).is_yes(),
            "no id in common is nothing to agree on"
        );

        let mut reg = TypeRegistry::new();
        let (w, r) = pair(
            &mut reg,
            MinimalTypeObject::Struct(
                TypeObjectBuilder::struct_type("::X")
                    .extensibility(Extensibility::Mutable)
                    .member("x1", i32t.clone(), |m| m.id(1))
                    .build_minimal(),
            ),
            MinimalTypeObject::Struct(
                TypeObjectBuilder::struct_type("::X")
                    .extensibility(Extensibility::Mutable)
                    .member("x1", i32t.clone(), |m| m.id(1))
                    .member("x2", i32t, |m| m.id(2))
                    .build_minimal(),
            ),
        );
        assert_eq!(
            is_assignable(&w, &r, &reg, &AssignabilityConfig::default()),
            Assignable::Yes,
            "one shared id carries the pair"
        );
    }

    fn bitmask_registry(bit_bound: u16) -> (TypeRegistry, TypeIdentifier) {
        use crate::type_object::flags::{BitflagFlag, BitmaskTypeFlag};
        use crate::type_object::minimal::{CommonBitflag, MinimalBitflag, MinimalBitmaskType};

        let obj = MinimalTypeObject::Bitmask(MinimalBitmaskType {
            bitmask_flags: BitmaskTypeFlag(0),
            bit_bound,
            flag_seq: alloc::vec![MinimalBitflag {
                common: CommonBitflag {
                    position: 0,
                    flags: BitflagFlag(0),
                },
                detail: crate::type_object::common::NameHash::from_name("flag0"),
            }],
        });
        let hash = compute_minimal_hash(&obj).unwrap();
        let mut reg = TypeRegistry::new();
        reg.insert_minimal(hash, obj);
        (reg, TypeIdentifier::EquivalenceHashMinimal(hash))
    }
}
