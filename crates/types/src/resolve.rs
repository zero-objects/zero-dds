// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Type-Resolution + Recursion-Guards (XTypes §7.3.4.5, §7.3.4.9).
//!
//! Cross-references between TypeObjects happen via `EquivalenceHash`
//! TypeIdentifiers (EK_MINIMAL / EK_COMPLETE). A TypeRegistry map caches
//! known objects; resolver functions follow alias chains and
//! detect recursion/cycles/DoS attempts via a depth cap.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use crate::type_identifier::{EquivalenceHash, TypeIdentifier};
use crate::type_object::minimal::MinimalTypeObject;
use crate::type_object::{CompleteTypeObject, TypeObject};

/// Maximum depth for recursively resolving alias chains and
/// TypeIdentifier references. Prevents DoS via malicious
/// type graphs with cycles.
pub const DEFAULT_MAX_RESOLVE_DEPTH: usize = 64;

/// Maximum number of nodes during [`collect_referenced_hashes`]. In addition
/// to the depth cap this also bounds wide/fan-out-heavy graphs
/// (a struct with 10,000 member entries that all reference hashes).
pub const DEFAULT_MAX_RESOLVE_NODES: usize = 4_096;

/// Error during type resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResolveError {
    /// Maximum depth exceeded.
    DepthExceeded {
        /// Allowed depth.
        limit: usize,
    },
    /// Maximum node count exceeded.
    NodeLimitExceeded {
        /// Allowed node count.
        limit: usize,
    },
    /// Referenced type not in the registry.
    Unknown {
        /// Hash that was looked up.
        hash: EquivalenceHash,
    },
    /// Cycle without progress detected.
    Cycle,
}

/// In-memory registry of known TypeObjects, indexed by
/// `EquivalenceHash`. Typically populated by TypeLookup replies.
#[derive(Debug, Clone, Default)]
pub struct TypeRegistry {
    minimals: BTreeMap<EquivalenceHash, MinimalTypeObject>,
    completes: BTreeMap<EquivalenceHash, CompleteTypeObject>,
}

impl TypeRegistry {
    /// Leere Registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a minimal TypeObject, indexed by its hash.
    pub fn insert_minimal(&mut self, hash: EquivalenceHash, t: MinimalTypeObject) {
        self.minimals.insert(hash, t);
    }

    /// Inserts a complete TypeObject.
    pub fn insert_complete(&mut self, hash: EquivalenceHash, t: CompleteTypeObject) {
        self.completes.insert(hash, t);
    }

    /// Lookup.
    #[must_use]
    pub fn get_minimal(&self, hash: &EquivalenceHash) -> Option<&MinimalTypeObject> {
        self.minimals.get(hash)
    }

    /// Lookup.
    #[must_use]
    pub fn get_complete(&self, hash: &EquivalenceHash) -> Option<&CompleteTypeObject> {
        self.completes.get(hash)
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> (usize, usize) {
        (self.minimals.len(), self.completes.len())
    }

    /// Empty?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.minimals.is_empty() && self.completes.is_empty()
    }

    /// Iterates over all minimal entries.
    pub fn iter_minimals(
        &self,
    ) -> alloc::collections::btree_map::Iter<'_, EquivalenceHash, MinimalTypeObject> {
        self.minimals.iter()
    }

    /// Iterates over all complete entries.
    pub fn iter_completes(
        &self,
    ) -> alloc::collections::btree_map::Iter<'_, EquivalenceHash, CompleteTypeObject> {
        self.completes.iter()
    }

    /// Returns all direct dependencies of a hash (transitively via
    /// [`collect_referenced_hashes`]). If the type is not in the
    /// registry, empty is returned.
    ///
    /// Prefers the Minimal variant, falls back to Complete.
    #[must_use]
    pub fn dependencies_of(&self, hash: &EquivalenceHash) -> Vec<EquivalenceHash> {
        let to = if let Some(m) = self.minimals.get(hash) {
            TypeObject::Minimal(m.clone())
        } else if let Some(c) = self.completes.get(hash) {
            TypeObject::Complete(c.clone())
        } else {
            return Vec::new();
        };
        collect_referenced_hashes(&to, DEFAULT_MAX_RESOLVE_DEPTH).unwrap_or_default()
    }

    /// Collects **transitively** all dependent hashes — follows each
    /// found hash recursively through the registry. The incoming hash
    /// itself is NOT included. Bounded by `max_nodes`.
    #[must_use]
    pub fn transitive_dependencies(
        &self,
        hash: &EquivalenceHash,
        max_nodes: usize,
    ) -> Vec<EquivalenceHash> {
        let mut out: Vec<EquivalenceHash> = Vec::new();
        let mut seen: BTreeSet<EquivalenceHash> = BTreeSet::new();
        let mut queue: Vec<EquivalenceHash> = self.dependencies_of(hash);
        while let Some(h) = queue.pop() {
            if !seen.insert(h) {
                continue;
            }
            if out.len() >= max_nodes {
                break;
            }
            out.push(h);
            for child in self.dependencies_of(&h) {
                if !seen.contains(&child) {
                    queue.push(child);
                }
            }
        }
        out
    }
}

/// Follows alias chains: if `ti` references an alias (and the
/// alias is known in the registry), resolve to the related_type.
/// Primitive / plain / hash-direct are returned unchanged.
///
/// # Errors
/// - `DepthExceeded` on deeper alias nesting than `max_depth`.
/// - `Unknown` if a hash is not in the registry.
pub fn resolve_alias_chain(
    start: &TypeIdentifier,
    registry: &TypeRegistry,
    max_depth: usize,
) -> Result<TypeIdentifier, ResolveError> {
    let mut current = start.clone();
    let mut visited: BTreeSet<EquivalenceHash> = BTreeSet::new();
    for _ in 0..max_depth {
        let hash = match &current {
            TypeIdentifier::EquivalenceHashMinimal(h) => *h,
            TypeIdentifier::EquivalenceHashComplete(h) => *h,
            _ => return Ok(current), // not hash-referenced — done
        };
        if !visited.insert(hash) {
            return Err(ResolveError::Cycle);
        }

        let next_ti = if matches!(current, TypeIdentifier::EquivalenceHashMinimal(_)) {
            match registry.get_minimal(&hash) {
                Some(MinimalTypeObject::Alias(a)) => a.body.common.related_type.clone(),
                Some(_) => return Ok(current), // struct/enum/... — not an alias
                None => return Err(ResolveError::Unknown { hash }),
            }
        } else {
            match registry.get_complete(&hash) {
                Some(CompleteTypeObject::Alias(a)) => a.body.related_type.clone(),
                Some(_) => return Ok(current),
                None => return Err(ResolveError::Unknown { hash }),
            }
        };
        current = next_ti;
    }
    Err(ResolveError::DepthExceeded { limit: max_depth })
}

/// Collects transitively all `EquivalenceHash` TypeIdentifiers that are
/// referenced by `root` directly or indirectly (through collections, struct
/// members, union cases, alias targets). Useful for
/// TypeLookup dependency resolution (T14).
///
/// Uses a depth cap against malicious type graphs.
///
/// # Errors
/// `DepthExceeded` if the type graph is deeper than `max_depth`.
pub fn collect_referenced_hashes(
    root: &TypeObject,
    max_depth: usize,
) -> Result<Vec<EquivalenceHash>, ResolveError> {
    let mut out = Vec::new();
    let mut seen = Vec::new();
    collect_from_type_object(root, max_depth, 0, &mut out, &mut seen)?;
    Ok(out)
}

fn collect_from_type_object(
    to: &TypeObject,
    max_depth: usize,
    depth: usize,
    out: &mut Vec<EquivalenceHash>,
    seen: &mut Vec<EquivalenceHash>,
) -> Result<(), ResolveError> {
    if depth >= max_depth {
        return Err(ResolveError::DepthExceeded { limit: max_depth });
    }
    match to {
        TypeObject::Minimal(m) => collect_from_minimal(m, max_depth, depth + 1, out, seen),
        TypeObject::Complete(c) => collect_from_complete(c, max_depth, depth + 1, out, seen),
    }
}

fn collect_from_minimal(
    m: &MinimalTypeObject,
    max_depth: usize,
    depth: usize,
    out: &mut Vec<EquivalenceHash>,
    seen: &mut Vec<EquivalenceHash>,
) -> Result<(), ResolveError> {
    match m {
        MinimalTypeObject::Struct(s) => {
            collect_from_ti(&s.header.base_type, max_depth, depth, out, seen)?;
            for mem in &s.member_seq {
                collect_from_ti(&mem.common.member_type_id, max_depth, depth, out, seen)?;
            }
        }
        MinimalTypeObject::Union(u) => {
            collect_from_ti(&u.discriminator.common.type_id, max_depth, depth, out, seen)?;
            for mem in &u.member_seq {
                collect_from_ti(&mem.common.type_id, max_depth, depth, out, seen)?;
            }
        }
        MinimalTypeObject::Alias(a) => {
            collect_from_ti(&a.body.common.related_type, max_depth, depth, out, seen)?;
        }
        MinimalTypeObject::Sequence(s) => {
            collect_from_ti(&s.element.common.type_id, max_depth, depth, out, seen)?;
        }
        MinimalTypeObject::Array(a) => {
            collect_from_ti(&a.element.common.type_id, max_depth, depth, out, seen)?;
        }
        MinimalTypeObject::Map(m) => {
            collect_from_ti(&m.key.common.type_id, max_depth, depth, out, seen)?;
            collect_from_ti(&m.element.common.type_id, max_depth, depth, out, seen)?;
        }
        _ => {} // Enum/Bitmask/Bitset/Annotation — no nested TIs
    }
    Ok(())
}

fn collect_from_complete(
    c: &CompleteTypeObject,
    max_depth: usize,
    depth: usize,
    out: &mut Vec<EquivalenceHash>,
    seen: &mut Vec<EquivalenceHash>,
) -> Result<(), ResolveError> {
    match c {
        CompleteTypeObject::Struct(s) => {
            collect_from_ti(&s.header.base_type, max_depth, depth, out, seen)?;
            for mem in &s.member_seq {
                collect_from_ti(&mem.common.member_type_id, max_depth, depth, out, seen)?;
            }
        }
        CompleteTypeObject::Union(u) => {
            collect_from_ti(&u.discriminator.common.type_id, max_depth, depth, out, seen)?;
            for mem in &u.member_seq {
                collect_from_ti(&mem.common.type_id, max_depth, depth, out, seen)?;
            }
        }
        CompleteTypeObject::Alias(a) => {
            collect_from_ti(&a.body.related_type, max_depth, depth, out, seen)?;
        }
        CompleteTypeObject::Sequence(s) => {
            collect_from_ti(&s.element.common.type_id, max_depth, depth, out, seen)?;
        }
        CompleteTypeObject::Array(a) => {
            collect_from_ti(&a.element.common.type_id, max_depth, depth, out, seen)?;
        }
        CompleteTypeObject::Map(m) => {
            collect_from_ti(&m.key.common.type_id, max_depth, depth, out, seen)?;
            collect_from_ti(&m.element.common.type_id, max_depth, depth, out, seen)?;
        }
        _ => {}
    }
    Ok(())
}

/// zerodds-lint: recursion-depth 16
///
/// Walks plain collection elements recursively (Sequence→Map→Array nesting).
/// The depth is effectively bounded by `TypeIdentifier::MAX_DECODE_DEPTH` (=16),
/// because the input comes from the wire decoder.
fn collect_from_ti(
    ti: &TypeIdentifier,
    _max_depth: usize,
    _depth: usize,
    out: &mut Vec<EquivalenceHash>,
    seen: &mut Vec<EquivalenceHash>,
) -> Result<(), ResolveError> {
    match ti {
        TypeIdentifier::EquivalenceHashMinimal(h) | TypeIdentifier::EquivalenceHashComplete(h) => {
            if !seen.contains(h) {
                if seen.len() >= DEFAULT_MAX_RESOLVE_NODES {
                    return Err(ResolveError::NodeLimitExceeded {
                        limit: DEFAULT_MAX_RESOLVE_NODES,
                    });
                }
                seen.push(*h);
                out.push(*h);
            }
        }
        TypeIdentifier::PlainSequenceSmall { element, .. }
        | TypeIdentifier::PlainSequenceLarge { element, .. }
        | TypeIdentifier::PlainArraySmall { element, .. }
        | TypeIdentifier::PlainArrayLarge { element, .. } => {
            collect_from_ti(element, _max_depth, _depth, out, seen)?;
        }
        TypeIdentifier::PlainMapSmall { element, key, .. }
        | TypeIdentifier::PlainMapLarge { element, key, .. } => {
            collect_from_ti(element, _max_depth, _depth, out, seen)?;
            collect_from_ti(key, _max_depth, _depth, out, seen)?;
        }
        _ => {}
    }
    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::builder::TypeObjectBuilder;
    use crate::hash::compute_minimal_hash;
    use crate::type_identifier::PrimitiveKind;
    use crate::type_object::minimal::MinimalAliasType;

    fn make_alias(related: TypeIdentifier) -> MinimalAliasType {
        use crate::type_object::flags::{AliasMemberFlag, AliasTypeFlag};
        use crate::type_object::minimal::{CommonAliasBody, MinimalAliasBody};
        MinimalAliasType {
            alias_flags: AliasTypeFlag::default(),
            body: MinimalAliasBody {
                common: CommonAliasBody {
                    related_flags: AliasMemberFlag::default(),
                    related_type: related,
                },
            },
        }
    }

    #[test]
    fn resolve_returns_primitive_unchanged() {
        let reg = TypeRegistry::new();
        let p = TypeIdentifier::Primitive(PrimitiveKind::Int64);
        let resolved = resolve_alias_chain(&p, &reg, 8).unwrap();
        assert_eq!(resolved, p);
    }

    #[test]
    fn resolve_follows_single_alias_to_target() {
        let mut reg = TypeRegistry::new();
        let target = TypeIdentifier::Primitive(PrimitiveKind::UInt64);
        let alias = MinimalTypeObject::Alias(make_alias(target.clone()));
        let alias_hash = compute_minimal_hash(&alias).unwrap();
        reg.insert_minimal(alias_hash, alias);

        let start = TypeIdentifier::EquivalenceHashMinimal(alias_hash);
        let resolved = resolve_alias_chain(&start, &reg, 8).unwrap();
        assert_eq!(resolved, target);
    }

    #[test]
    fn resolve_chained_aliases_two_deep() {
        let mut reg = TypeRegistry::new();
        let final_target = TypeIdentifier::Primitive(PrimitiveKind::Int8);
        let inner = MinimalTypeObject::Alias(make_alias(final_target.clone()));
        let inner_hash = compute_minimal_hash(&inner).unwrap();
        reg.insert_minimal(inner_hash, inner);

        let outer = MinimalTypeObject::Alias(make_alias(TypeIdentifier::EquivalenceHashMinimal(
            inner_hash,
        )));
        let outer_hash = compute_minimal_hash(&outer).unwrap();
        reg.insert_minimal(outer_hash, outer);

        let resolved =
            resolve_alias_chain(&TypeIdentifier::EquivalenceHashMinimal(outer_hash), &reg, 8)
                .unwrap();
        assert_eq!(resolved, final_target);
    }

    #[test]
    fn resolve_cycle_detected() {
        let mut reg = TypeRegistry::new();
        // Two aliases pointing at each other. To get consistent hashes
        // we would have to insert them simultaneously — we simulate
        // a cycle by registering the same alias with related_type = its
        // own hash.
        let self_hash = EquivalenceHash([0x99; 14]);
        let self_alias = MinimalTypeObject::Alias(make_alias(
            TypeIdentifier::EquivalenceHashMinimal(self_hash),
        ));
        reg.insert_minimal(self_hash, self_alias);

        let err = resolve_alias_chain(&TypeIdentifier::EquivalenceHashMinimal(self_hash), &reg, 8)
            .unwrap_err();
        assert_eq!(err, ResolveError::Cycle);
    }

    #[test]
    fn resolve_unknown_hash_fails() {
        let reg = TypeRegistry::new();
        let err = resolve_alias_chain(
            &TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash([0xAA; 14])),
            &reg,
            8,
        )
        .unwrap_err();
        assert!(matches!(err, ResolveError::Unknown { .. }));
    }

    #[test]
    fn collect_referenced_hashes_picks_up_nested_struct_members() {
        let inner_hash = EquivalenceHash([0x01; 14]);
        let st = TypeObjectBuilder::struct_type("::X")
            .member(
                "ref_a",
                TypeIdentifier::EquivalenceHashMinimal(inner_hash),
                |m| m,
            )
            .member("p", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
            .build_minimal();
        let to = TypeObject::Minimal(MinimalTypeObject::Struct(st));
        let refs = collect_referenced_hashes(&to, DEFAULT_MAX_RESOLVE_DEPTH).unwrap();
        assert_eq!(refs, alloc::vec![inner_hash]);
    }

    #[test]
    fn collect_referenced_hashes_from_union_cases() {
        use crate::type_object::common::CommonUnionMember;
        use crate::type_object::flags::{UnionDiscriminatorFlag, UnionMemberFlag, UnionTypeFlag};
        use crate::type_object::minimal::{
            CommonDiscriminatorMember, MinimalDiscriminatorMember, MinimalUnionMember,
            MinimalUnionType,
        };
        let h_disc = EquivalenceHash([0x01; 14]);
        let h_case = EquivalenceHash([0x02; 14]);
        let u = MinimalUnionType {
            union_flags: UnionTypeFlag::default(),
            discriminator: MinimalDiscriminatorMember {
                common: CommonDiscriminatorMember {
                    member_flags: UnionDiscriminatorFlag::default(),
                    type_id: TypeIdentifier::EquivalenceHashMinimal(h_disc),
                },
            },
            member_seq: alloc::vec![MinimalUnionMember {
                common: CommonUnionMember {
                    member_id: 1,
                    member_flags: UnionMemberFlag::default(),
                    type_id: TypeIdentifier::EquivalenceHashMinimal(h_case),
                    label_seq: alloc::vec![1],
                },
                detail: crate::type_object::common::NameHash([0; 4]),
            }],
        };
        let to = TypeObject::Minimal(MinimalTypeObject::Union(u));
        let refs = collect_referenced_hashes(&to, DEFAULT_MAX_RESOLVE_DEPTH).unwrap();
        assert!(refs.contains(&h_disc));
        assert!(refs.contains(&h_case));
    }

    #[test]
    fn collect_referenced_hashes_from_sequence_and_array_and_map() {
        use crate::builder::TypeObjectBuilder;
        let h_elem = EquivalenceHash([0x10; 14]);

        let seq = TypeObjectBuilder::sequence(TypeIdentifier::EquivalenceHashMinimal(h_elem), 50)
            .build_minimal();
        let refs = collect_referenced_hashes(
            &TypeObject::Minimal(MinimalTypeObject::Sequence(seq)),
            DEFAULT_MAX_RESOLVE_DEPTH,
        )
        .unwrap();
        assert_eq!(refs, alloc::vec![h_elem]);

        let arr = TypeObjectBuilder::array(
            TypeIdentifier::EquivalenceHashMinimal(h_elem),
            alloc::vec![2, 3],
        )
        .build_minimal();
        let refs = collect_referenced_hashes(
            &TypeObject::Minimal(MinimalTypeObject::Array(arr)),
            DEFAULT_MAX_RESOLVE_DEPTH,
        )
        .unwrap();
        assert_eq!(refs, alloc::vec![h_elem]);

        let h_key = EquivalenceHash([0x20; 14]);
        let map = TypeObjectBuilder::map(
            TypeIdentifier::EquivalenceHashMinimal(h_key),
            TypeIdentifier::EquivalenceHashMinimal(h_elem),
            10,
        )
        .build_minimal();
        let refs = collect_referenced_hashes(
            &TypeObject::Minimal(MinimalTypeObject::Map(map)),
            DEFAULT_MAX_RESOLVE_DEPTH,
        )
        .unwrap();
        assert!(refs.contains(&h_key));
        assert!(refs.contains(&h_elem));
    }

    #[test]
    fn collect_referenced_hashes_from_complete_typeobject() {
        use crate::builder::TypeObjectBuilder;
        let h = EquivalenceHash([0x44; 14]);
        let st = TypeObjectBuilder::struct_type("::X")
            .member("f", TypeIdentifier::EquivalenceHashMinimal(h), |m| m)
            .build_complete();
        let to = TypeObject::Complete(CompleteTypeObject::Struct(st));
        let refs = collect_referenced_hashes(&to, DEFAULT_MAX_RESOLVE_DEPTH).unwrap();
        assert_eq!(refs, alloc::vec![h]);
    }

    #[test]
    fn collect_referenced_hashes_from_alias() {
        use crate::builder::TypeObjectBuilder;
        let h = EquivalenceHash([0x55; 14]);
        let a = TypeObjectBuilder::alias("::A", TypeIdentifier::EquivalenceHashMinimal(h))
            .build_minimal();
        let to = TypeObject::Minimal(MinimalTypeObject::Alias(a));
        let refs = collect_referenced_hashes(&to, DEFAULT_MAX_RESOLVE_DEPTH).unwrap();
        assert_eq!(refs, alloc::vec![h]);
    }

    #[test]
    fn collect_referenced_hashes_nested_plain_sequence_in_plain_map() {
        // Plain TypeIdentifier im Member-Type: nested Sequence<of Map<of Hash>>
        let h = EquivalenceHash([0x66; 14]);
        let inner_map = TypeIdentifier::PlainMapSmall {
            header: crate::type_identifier::PlainCollectionHeader::default(),
            bound: 1,
            element: alloc::boxed::Box::new(TypeIdentifier::EquivalenceHashMinimal(h)),
            key_flags: crate::type_identifier::CollectionElementFlag(0),
            key: alloc::boxed::Box::new(TypeIdentifier::Primitive(
                crate::type_identifier::PrimitiveKind::Int32,
            )),
        };
        let outer_seq = TypeIdentifier::PlainSequenceSmall {
            header: crate::type_identifier::PlainCollectionHeader::default(),
            bound: 3,
            element: alloc::boxed::Box::new(inner_map),
        };
        use crate::builder::TypeObjectBuilder;
        let st = TypeObjectBuilder::struct_type("::Nested")
            .member("data", outer_seq, |m| m)
            .build_minimal();
        let to = TypeObject::Minimal(MinimalTypeObject::Struct(st));
        let refs = collect_referenced_hashes(&to, DEFAULT_MAX_RESOLVE_DEPTH).unwrap();
        assert_eq!(refs, alloc::vec![h]);
    }

    #[test]
    fn resolve_depth_exceeded_with_long_alias_chain() {
        // Build chain depth > max_depth.
        let mut reg = TypeRegistry::new();
        let mut last_hash = EquivalenceHash([0x00; 14]);
        for i in 0..10u8 {
            let target = if i == 0 {
                TypeIdentifier::Primitive(crate::type_identifier::PrimitiveKind::Int32)
            } else {
                TypeIdentifier::EquivalenceHashMinimal(last_hash)
            };
            let alias = MinimalTypeObject::Alias(make_alias(target));
            let h = crate::hash::compute_minimal_hash(&alias).unwrap();
            reg.insert_minimal(h, alias);
            last_hash = h;
        }
        let err = resolve_alias_chain(
            &TypeIdentifier::EquivalenceHashMinimal(last_hash),
            &reg,
            3, // depth too low
        )
        .unwrap_err();
        assert_eq!(err, ResolveError::DepthExceeded { limit: 3 });
    }

    #[test]
    fn collect_referenced_hashes_skips_duplicates() {
        let h = EquivalenceHash([0x22; 14]);
        let st = TypeObjectBuilder::struct_type("::Y")
            .member("a", TypeIdentifier::EquivalenceHashMinimal(h), |m| m)
            .member("b", TypeIdentifier::EquivalenceHashMinimal(h), |m| m)
            .build_minimal();
        let to = TypeObject::Minimal(MinimalTypeObject::Struct(st));
        let refs = collect_referenced_hashes(&to, DEFAULT_MAX_RESOLVE_DEPTH).unwrap();
        assert_eq!(refs.len(), 1);
    }

    #[test]
    fn collect_referenced_hashes_node_limit_exceeded_on_wide_struct() {
        // A struct with DEFAULT_MAX_RESOLVE_NODES + 1 distinct hash refs
        // triggers the node-limit brake.
        let mut builder = TypeObjectBuilder::struct_type("::Wide");
        for i in 0..(DEFAULT_MAX_RESOLVE_NODES as u32 + 1) {
            let mut h_bytes = [0u8; 14];
            h_bytes[..4].copy_from_slice(&i.to_le_bytes());
            let h = EquivalenceHash(h_bytes);
            builder = builder.member(
                alloc::format!("m_{i}").as_str(),
                TypeIdentifier::EquivalenceHashMinimal(h),
                |m| m,
            );
        }
        let st = builder.build_minimal();
        let to = TypeObject::Minimal(MinimalTypeObject::Struct(st));
        let err = collect_referenced_hashes(&to, DEFAULT_MAX_RESOLVE_DEPTH).unwrap_err();
        assert_eq!(
            err,
            ResolveError::NodeLimitExceeded {
                limit: DEFAULT_MAX_RESOLVE_NODES,
            }
        );
    }

    // ====================================================================
    // SCC / Mutual-Dependency (XTypes 1.3 §7.3.4.8 + §7.3.4.9)
    // ====================================================================
    //
    // When three or more structs reference each other, they form
    // a strongly connected component (SCC). The TypeRegistry must
    // perform reference resolution correctly despite the cycle — the seen-set-
    // based walker prevents infinite loops, and transitive_dependencies
    // deterministically returns all 3/4/N nodes of the SCC.

    fn struct_with_member_ref(name: &str, ref_hash: EquivalenceHash) -> MinimalTypeObject {
        MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type(name)
                .member(
                    "next",
                    TypeIdentifier::EquivalenceHashMinimal(ref_hash),
                    |m| m,
                )
                .build_minimal(),
        )
    }

    #[test]
    fn scc_three_element_cycle_resolves_correctly() {
        // A -> B -> C -> A
        let h_a = EquivalenceHash([0x0A; 14]);
        let h_b = EquivalenceHash([0x0B; 14]);
        let h_c = EquivalenceHash([0x0C; 14]);
        let mut reg = TypeRegistry::new();
        reg.insert_minimal(h_a, struct_with_member_ref("A", h_b));
        reg.insert_minimal(h_b, struct_with_member_ref("B", h_c));
        reg.insert_minimal(h_c, struct_with_member_ref("C", h_a));

        let deps = reg.transitive_dependencies(&h_a, DEFAULT_MAX_RESOLVE_NODES);
        assert!(deps.contains(&h_a));
        assert!(deps.contains(&h_b));
        assert!(deps.contains(&h_c));
        // No infinite loop: all 3 + a-self-ref onto known hashes
        // → exactly 3 unique entries.
        let mut sorted = deps.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 3);
    }

    #[test]
    fn scc_four_element_cycle_resolves_correctly() {
        // A -> B -> C -> D -> A (4-node cycle)
        let h_a = EquivalenceHash([0x1A; 14]);
        let h_b = EquivalenceHash([0x1B; 14]);
        let h_c = EquivalenceHash([0x1C; 14]);
        let h_d = EquivalenceHash([0x1D; 14]);
        let mut reg = TypeRegistry::new();
        reg.insert_minimal(h_a, struct_with_member_ref("A", h_b));
        reg.insert_minimal(h_b, struct_with_member_ref("B", h_c));
        reg.insert_minimal(h_c, struct_with_member_ref("C", h_d));
        reg.insert_minimal(h_d, struct_with_member_ref("D", h_a));

        let deps = reg.transitive_dependencies(&h_a, DEFAULT_MAX_RESOLVE_NODES);
        assert!(deps.contains(&h_a));
        assert!(deps.contains(&h_b));
        assert!(deps.contains(&h_c));
        assert!(deps.contains(&h_d));
        let mut sorted = deps.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 4);
    }

    #[test]
    fn scc_diamond_with_cycle_resolves_finitely() {
        //   A -> B -> D
        //   |    |
        //   v    v
        //   C -> A   (diamond with a back-edge to A)
        let h_a = EquivalenceHash([0x2A; 14]);
        let h_b = EquivalenceHash([0x2B; 14]);
        let h_c = EquivalenceHash([0x2C; 14]);
        let h_d = EquivalenceHash([0x2D; 14]);
        let mut reg = TypeRegistry::new();
        // A has two members: -> B and -> C
        let st_a = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("A")
                .member("to_b", TypeIdentifier::EquivalenceHashMinimal(h_b), |m| m)
                .member("to_c", TypeIdentifier::EquivalenceHashMinimal(h_c), |m| m)
                .build_minimal(),
        );
        reg.insert_minimal(h_a, st_a);
        reg.insert_minimal(h_b, struct_with_member_ref("B", h_d));
        reg.insert_minimal(h_c, struct_with_member_ref("C", h_a));
        // D: terminal (no hash member)
        let st_d = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("D")
                .member("x", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .build_minimal(),
        );
        reg.insert_minimal(h_d, st_d);

        let deps = reg.transitive_dependencies(&h_a, DEFAULT_MAX_RESOLVE_NODES);
        let mut sorted = deps.clone();
        sorted.sort();
        sorted.dedup();
        assert!(sorted.contains(&h_a));
        assert!(sorted.contains(&h_b));
        assert!(sorted.contains(&h_c));
        assert!(sorted.contains(&h_d));
        assert_eq!(sorted.len(), 4);
    }

    // ---- §7.2.2.4.9 @external Forward-Decls + Diamond ----

    #[test]
    fn external_forward_decl_roundtrip() {
        // Struct A has an `@external A* next` (self-forward).
        // The TypeObject's wire encode/decode preserves the EXTERNAL flag.
        use crate::type_identifier::PrimitiveKind;
        use crate::type_object::TypeObject;
        use crate::type_object::flags::StructMemberFlag;
        use crate::type_object::minimal::MinimalTypeObject;
        let h_self = EquivalenceHash([0x4A; 14]);
        let mut a = TypeObjectBuilder::struct_type("::A")
            .member(
                "next",
                TypeIdentifier::EquivalenceHashMinimal(h_self),
                |m| m.external().id(1),
            )
            .member(
                "value",
                TypeIdentifier::Primitive(PrimitiveKind::Int32),
                |m| m.id(2),
            )
            .build_minimal();
        // Pin the hash to the wire form (only for the test).
        a.member_seq[0].common.member_flags = StructMemberFlag(StructMemberFlag::IS_EXTERNAL);

        let mut reg = TypeRegistry::new();
        reg.insert_minimal(h_self, MinimalTypeObject::Struct(a.clone()));

        // Wire-Roundtrip via TypeObject Encode/Decode.
        let to = TypeObject::Minimal(MinimalTypeObject::Struct(a.clone()));
        let bytes = to.to_bytes_le().unwrap();
        let decoded = TypeObject::from_bytes_le(&bytes).unwrap();
        if let TypeObject::Minimal(MinimalTypeObject::Struct(b)) = decoded {
            assert!(
                b.member_seq[0]
                    .common
                    .member_flags
                    .has(StructMemberFlag::IS_EXTERNAL)
            );
            assert_eq!(b.member_seq[0].common.member_id, 1);
        } else {
            panic!("expected MinimalStruct");
        }

        // Forward resolution via the registry: dependencies_of returns h_self.
        let deps = reg.transitive_dependencies(&h_self, DEFAULT_MAX_RESOLVE_NODES);
        assert!(deps.contains(&h_self));
    }

    #[test]
    fn external_diamond_resolves_through_both_paths() {
        // Diamond:
        //   A -> @external B -> @external D
        //   A -> @external C -> @external D
        //
        // Expected: transitive_dependencies(A) = {B, C, D}, each unique.
        use crate::type_identifier::PrimitiveKind;
        use crate::type_object::flags::StructMemberFlag;
        use crate::type_object::minimal::MinimalTypeObject;
        let h_a = EquivalenceHash([0x5A; 14]);
        let h_b = EquivalenceHash([0x5B; 14]);
        let h_c = EquivalenceHash([0x5C; 14]);
        let h_d = EquivalenceHash([0x5D; 14]);

        let mut a = TypeObjectBuilder::struct_type("A")
            .member("to_b", TypeIdentifier::EquivalenceHashMinimal(h_b), |m| {
                m.external().id(1)
            })
            .member("to_c", TypeIdentifier::EquivalenceHashMinimal(h_c), |m| {
                m.external().id(2)
            })
            .build_minimal();
        // Ensure that the EXTERNAL flags are actually set.
        for m in &mut a.member_seq {
            m.common.member_flags = StructMemberFlag(StructMemberFlag::IS_EXTERNAL);
        }

        let b = TypeObjectBuilder::struct_type("B")
            .member("to_d", TypeIdentifier::EquivalenceHashMinimal(h_d), |m| {
                m.external().id(1)
            })
            .build_minimal();
        let c = TypeObjectBuilder::struct_type("C")
            .member("to_d", TypeIdentifier::EquivalenceHashMinimal(h_d), |m| {
                m.external().id(1)
            })
            .build_minimal();
        let d = TypeObjectBuilder::struct_type("D")
            .member("v", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.id(1)
            })
            .build_minimal();

        let mut reg = TypeRegistry::new();
        reg.insert_minimal(h_a, MinimalTypeObject::Struct(a));
        reg.insert_minimal(h_b, MinimalTypeObject::Struct(b));
        reg.insert_minimal(h_c, MinimalTypeObject::Struct(c));
        reg.insert_minimal(h_d, MinimalTypeObject::Struct(d));

        let deps = reg.transitive_dependencies(&h_a, DEFAULT_MAX_RESOLVE_NODES);
        let mut sorted = deps.clone();
        sorted.sort();
        sorted.dedup();
        assert!(sorted.contains(&h_b));
        assert!(sorted.contains(&h_c));
        assert!(sorted.contains(&h_d));
        // 3 unique hashes (B, C, D); A itself is the root.
        assert_eq!(sorted.len(), 3);
    }

    #[test]
    fn scc_self_loop_does_not_explode_node_count() {
        // A -> A (1-node cycle, only self-loop).
        let h_a = EquivalenceHash([0x3A; 14]);
        let mut reg = TypeRegistry::new();
        reg.insert_minimal(h_a, struct_with_member_ref("A", h_a));

        let deps = reg.transitive_dependencies(&h_a, DEFAULT_MAX_RESOLVE_NODES);
        // dependencies_of(h_a) = [h_a], queue.pop() → seen.insert(h_a) push,
        // then children of h_a = [h_a] already in seen → done. Exactly 1 entry.
        assert_eq!(deps, alloc::vec![h_a]);
    }
}
