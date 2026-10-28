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
    /// A hash-only dependency list cannot represent an SCC member identity.
    UnsupportedComponentReference,
    /// An extended or otherwise unsupported identifier may contain references
    /// that this revision cannot enumerate faithfully.
    UnsupportedReference,
}

/// A strongly connected component in both forms: the objects of its
/// members in the order XTypes 1.3 §7.3.4.9.2 step 4b fixes, and the
/// hash each form is identified by.
///
/// What a recursive topic type hands the runtime so that it can register
/// the whole component: one object cannot be filed on its own, because the
/// hash that identifies a member belongs to the component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeComponent {
    /// The hash of the COMPLETE component.
    pub complete_hash: EquivalenceHash,
    /// The hash of the MINIMAL component.
    pub minimal_hash: EquivalenceHash,
    /// The COMPLETE members, by `scc_index - 1`.
    pub complete: Vec<CompleteTypeObject>,
    /// The MINIMAL members, by `scc_index - 1`.
    pub minimal: Vec<MinimalTypeObject>,
}

/// In-memory registry of known TypeObjects, indexed by
/// `EquivalenceHash`. Typically populated by TypeLookup replies.
#[derive(Debug, Clone, Default)]
pub struct TypeRegistry {
    minimals: BTreeMap<EquivalenceHash, MinimalTypeObject>,
    completes: BTreeMap<EquivalenceHash, CompleteTypeObject>,
    /// The members of each strongly connected component, in the order
    /// XTypes 1.3 §7.3.4.9.2 step 4b fixes.
    ///
    /// A `TI_STRONGLY_CONNECTED_COMPONENT` identifier names a hash and a
    /// position within it, so a reader that holds only hash-to-object
    /// pairs cannot follow one: the hash belongs to the component, not
    /// to any single type in it. This is what makes the position
    /// resolvable.
    components: BTreeMap<EquivalenceHash, Vec<CompleteTypeObject>>,
    /// The same components in their MINIMAL form, keyed by the hash of the
    /// minimal component.
    ///
    /// §7.3.4.9.2 runs the algorithm once per equivalence kind: the
    /// MINIMAL component has its own TypeObjects (annotations and names
    /// left out), its own hash, and the same members at the same
    /// positions. The two hashes are unrelated, and a COMPLETE component
    /// cannot be projected onto a MINIMAL one by this registry's
    /// `to_minimal` (a `TI_STRONGLY_CONNECTED_COMPONENT` reference is not
    /// projected, see `type_object::to_minimal`), so the minimal members
    /// are filed when they are built.
    minimal_components: BTreeMap<EquivalenceHash, Vec<MinimalTypeObject>>,
    /// The MINIMAL component hash of each COMPLETE component hash.
    minimal_component_of: BTreeMap<EquivalenceHash, EquivalenceHash>,
    /// The pairs of component members the assignability walk is inside of.
    ///
    /// A recursive type reaches itself, and so does the question whether
    /// one recursive type is assignable from another: the answer for a
    /// pair that is met again while it is being decided is the one
    /// already assumed (the walk is coinductive, a cycle that finds no
    /// contradiction is consistent). The set lives here because the
    /// registry is the one argument every step of the walk is handed.
    assumed: core::cell::RefCell<Vec<(TypeIdentifier, TypeIdentifier)>>,
    /// Whether this registry projects complete objects to their minimal
    /// form in the RTI Connext/CoreDX camp (one trailing octet per
    /// minimal aggregated member) rather than the default Cyclone/Fast
    /// DDS one. Set once from the participant's configuration; carried
    /// into every `to_minimal` this registry drives, so the minimal hash
    /// it announces and the one it serves agree. `docs/omg-xtypes-issues.md`
    /// §C2.
    minimal_member_trailing_octet: bool,
    /// Bumped by every insertion (direct or via [`Self::project_completes_to_minimal`]),
    /// never by a lookup. A caller that cached "this hash is not here yet"
    /// compares its own last-seen value against [`Self::generation`] to
    /// tell a stale cache from a registry that has not grown, without
    /// re-querying the map on every check.
    generation: u64,
}

impl TypeRegistry {
    /// Empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Choose the minimal-`TypeObject` vendor camp this registry projects
    /// in. `false` (default) is Cyclone/Fast DDS; `true` RTI/CoreDX.
    /// `docs/omg-xtypes-issues.md` §C2.
    pub fn set_minimal_member_trailing_octet(&mut self, on: bool) {
        self.minimal_member_trailing_octet = on;
    }

    /// Whether this registry projects in the RTI/CoreDX minimal camp.
    #[must_use]
    pub fn minimal_member_trailing_octet(&self) -> bool {
        self.minimal_member_trailing_octet
    }

    /// Inserts a minimal TypeObject, indexed by its hash.
    pub fn insert_minimal(&mut self, hash: EquivalenceHash, t: MinimalTypeObject) {
        self.minimals.insert(hash, t);
        self.generation = self.generation.wrapping_add(1);
    }

    /// Records a strongly connected component's members, in step 4b
    /// order, under the component's hash.
    ///
    /// Replaces any previous entry for that hash: a component is a whole
    /// or it is nothing, and merging two partial lists would put a type
    /// at an index that is not its own.
    pub fn insert_component(&mut self, hash: EquivalenceHash, members: Vec<CompleteTypeObject>) {
        self.components.insert(hash, members);
        self.generation = self.generation.wrapping_add(1);
    }

    /// How many insertions this registry has seen (direct, or via
    /// [`Self::project_completes_to_minimal`]), monotonically. Never
    /// decreases and never means anything about the registry's
    /// current size on its own — only whether it changed since a
    /// caller last read it.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Decide `walk` with the pair (`writer`, `reader`) assumed assignable,
    /// or `None` when the pair is already being decided further up.
    ///
    /// Two recursive types are compared member by member and the members
    /// point back at each other, so the walk meets the same pair again.
    /// Such a pair is taken as the answer it is being given, and a cycle
    /// that finds no contradiction is consistent: the caller treats `None`
    /// as `Yes`.
    pub(crate) fn assuming<T>(
        &self,
        writer: &TypeIdentifier,
        reader: &TypeIdentifier,
        walk: impl FnOnce() -> T,
    ) -> Option<T> {
        {
            let mut assumed = self.assumed.borrow_mut();
            if assumed.iter().any(|(w, r)| w == writer && r == reader) {
                return None;
            }
            assumed.push((writer.clone(), reader.clone()));
        }
        let outcome = walk();
        self.assumed.borrow_mut().pop();
        Some(outcome)
    }

    /// Records the MINIMAL members of the component whose COMPLETE hash is
    /// `complete_hash`, under their own hash, in step 4b order.
    ///
    /// As [`Self::insert_component`]: a component is a whole or nothing.
    pub fn insert_minimal_component(
        &mut self,
        complete_hash: EquivalenceHash,
        minimal_hash: EquivalenceHash,
        members: Vec<MinimalTypeObject>,
    ) {
        self.minimal_components.insert(minimal_hash, members);
        self.minimal_component_of
            .insert(complete_hash, minimal_hash);
        self.generation = self.generation.wrapping_add(1);
    }

    /// The MINIMAL hash of the component whose COMPLETE hash is `hash`.
    #[must_use]
    pub fn minimal_component_hash(&self, hash: &EquivalenceHash) -> Option<EquivalenceHash> {
        self.minimal_component_of.get(hash).copied()
    }

    /// The COMPLETE hash of the component whose MINIMAL hash is
    /// `minimal_hash`, when the registry filed the pair.
    #[must_use]
    pub fn complete_component_hash(
        &self,
        minimal_hash: &EquivalenceHash,
    ) -> Option<EquivalenceHash> {
        self.minimal_component_of
            .iter()
            .find(|(_, minimal)| *minimal == minimal_hash)
            .map(|(complete, _)| *complete)
    }

    /// All MINIMAL members of the component `minimal_hash` names, in step
    /// 4b order; `None` when the component is unknown.
    #[must_use]
    pub fn minimal_component_members(
        &self,
        minimal_hash: &EquivalenceHash,
    ) -> Option<&[MinimalTypeObject]> {
        self.minimal_components.get(minimal_hash).map(Vec::as_slice)
    }

    /// The MINIMAL object a `TI_STRONGLY_CONNECTED_COMPONENT` identifier
    /// of either kind stands for: the member at its position of the
    /// minimal component, which for a COMPLETE identifier is the one that
    /// shares its position with the complete component.
    ///
    /// `None` for an `SCCIdentifier` (position 0), an unknown component or
    /// a position outside it.
    #[must_use]
    pub fn component_minimal_member(
        &self,
        scc: &crate::type_identifier::StronglyConnectedComponentId,
    ) -> Option<&MinimalTypeObject> {
        let position = usize::try_from(scc.scc_index.checked_sub(1)?).ok()?;
        let minimal_hash = if scc.equivalence_kind == crate::type_identifier::kinds::EK_COMPLETE {
            *self.minimal_component_of.get(&scc.hash)?
        } else {
            scc.hash
        };
        self.minimal_components.get(&minimal_hash)?.get(position)
    }

    /// The member at `scc_index` of the component `hash` names.
    /// The member at `scc_index` of the component `hash` names.
    ///
    /// `scc_index` is the identifier's own, which §7.3.4.9.2 step 4b
    /// numbers from one; `None` when the component is unknown or the
    /// index is outside it.
    #[must_use]
    pub fn component_member(
        &self,
        hash: &EquivalenceHash,
        scc_index: i32,
    ) -> Option<&CompleteTypeObject> {
        let members = self.components.get(hash)?;
        let position = usize::try_from(scc_index.checked_sub(1)?).ok()?;
        members.get(position)
    }

    /// All members of the component `hash` names, in step 4b order.
    ///
    /// `None` when the component is unknown. The members are returned as
    /// a whole: §7.6.3.3 has a getTypes answer treat the TypeObjects of a
    /// strongly connected component atomically.
    #[must_use]
    pub fn component_members(&self, hash: &EquivalenceHash) -> Option<&[CompleteTypeObject]> {
        self.components.get(hash).map(Vec::as_slice)
    }

    /// Inserts a complete TypeObject.
    pub fn insert_complete(&mut self, hash: EquivalenceHash, t: CompleteTypeObject) {
        self.completes.insert(hash, t);
        self.generation = self.generation.wrapping_add(1);
    }

    /// Records a `TypeObject` under the hash of its own serialization.
    ///
    /// The hash is computed here rather than taken from the caller: a
    /// registry keyed by what a peer *said* an object hashes to hands out
    /// an object under an identifier that does not describe it, and every
    /// assignability verdict drawn from it is then about the wrong type.
    /// A `TypeLookup_getTypes_Out` carries the identifier beside the
    /// object (XTypes 1.3 §7.6.3.3.3) and that identifier is a claim.
    ///
    /// The match over the equivalence kind lives here, in the crate that
    /// owns the enum, so no caller outside it needs a wildcard arm —
    /// which is what a silently dropped TypeObject would look like.
    ///
    /// # Errors
    /// `WireError` if the object does not serialize.
    pub fn insert_object(
        &mut self,
        object: crate::type_object::TypeObject,
    ) -> Result<EquivalenceHash, zerodds_wire::WireError> {
        let hash = crate::hash::compute_hash(&object)?;
        match object {
            crate::type_object::TypeObject::Minimal(m) => self.insert_minimal(hash, m),
            crate::type_object::TypeObject::Complete(c) => self.insert_complete(hash, c),
        }
        Ok(hash)
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
    /// Prefers the Minimal variant, falls back to Complete. This convenience
    /// API maps collection errors to an empty list; it does not certify a
    /// complete dependency closure. Use [`collect_referenced_hashes`] and
    /// handle its errors for discovery metadata.
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
    /// itself is NOT included. Bounded by `max_nodes`. This convenience API
    /// truncates at that limit and inherits [`Self::dependencies_of`]'s error
    /// fallback; do not use it to certify complete discovery metadata.
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

    /// Whether the registry can resolve a component identifier of either
    /// kind: the members of that kind are filed, and for a COMPLETE
    /// identifier the minimal component beside them.
    #[must_use]
    pub fn holds_component(
        &self,
        scc: &crate::type_identifier::StronglyConnectedComponentId,
    ) -> bool {
        if scc.equivalence_kind == crate::type_identifier::kinds::EK_COMPLETE {
            self.components.contains_key(&scc.hash)
                && self
                    .minimal_component_of
                    .get(&scc.hash)
                    .is_some_and(|minimal| self.minimal_components.contains_key(minimal))
        } else {
            self.minimal_components.contains_key(&scc.hash)
        }
    }

    /// Files a component in both forms (see [`TypeComponent`]).
    pub fn insert_type_component(&mut self, component: &TypeComponent) {
        self.insert_component(component.complete_hash, component.complete.clone());
        self.insert_minimal_component(
            component.complete_hash,
            component.minimal_hash,
            component.minimal.clone(),
        );
    }

    /// The hash identifiers the members of the component `complete_hash`
    /// names, transitively, each once — the dependencies of a component
    /// that are types of their own (§7.6.3.3.4.1). The component's own
    /// members and the other components its members reach are not in the
    /// list: those are named by their `SCCIdentifier`.
    #[must_use]
    pub fn component_dependencies(&self, complete_hash: &EquivalenceHash) -> Vec<EquivalenceHash> {
        let Some(members) = self.components.get(complete_hash) else {
            return Vec::new();
        };
        let mut queue: Vec<EquivalenceHash> = Vec::new();
        for member in members {
            for identifier in named_identifiers(member).unwrap_or_default() {
                if let TypeIdentifier::EquivalenceHashComplete(hash) = identifier {
                    queue.push(*hash);
                }
            }
        }
        let mut out: Vec<EquivalenceHash> = Vec::new();
        let mut seen: BTreeSet<EquivalenceHash> = BTreeSet::new();
        while let Some(hash) = queue.pop() {
            if !seen.insert(hash) {
                continue;
            }
            out.push(hash);
            for child in self.transitive_dependencies(&hash, DEFAULT_MAX_RESOLVE_NODES) {
                if seen.insert(child) {
                    out.push(child);
                }
            }
        }
        out
    }

    /// Files the components a `getTypes` reply carries, whole or not at
    /// all, and returns how many were filed.
    ///
    /// §7.6.3.3: a reply treats "the TypeObjects within the Strongly
    /// Connected Components atomically. Either include all in the reply or
    /// none" — each member under the `TI_STRONGLY_CONNECTED_COMPONENT`
    /// identifier of its own position. A hash of an object cannot be the
    /// key of a member (the component's hash belongs to all of them), so
    /// `insert_object` cannot file one, and a component with a position
    /// missing is dropped rather than filed with a type at an index that
    /// is not its own.
    ///
    /// The MINIMAL component is filed beside the COMPLETE one it is paired
    /// with in `complete_to_minimal` (§7.6.3.3.4.2); a minimal component
    /// the reply does not pair is filed under its own hash only, which is
    /// what an identifier of that kind resolves by.
    pub fn insert_components_of(&mut self, reply: &crate::type_lookup::GetTypesReply) -> usize {
        use crate::type_identifier::kinds::{EK_COMPLETE, EK_MINIMAL};
        let mut complete: BTreeMap<EquivalenceHash, (i32, BTreeMap<i32, CompleteTypeObject>)> =
            BTreeMap::new();
        let mut minimal: BTreeMap<EquivalenceHash, (i32, BTreeMap<i32, MinimalTypeObject>)> =
            BTreeMap::new();
        for pair in &reply.types {
            let TypeIdentifier::StronglyConnectedComponent(scc) = &pair.type_identifier else {
                continue;
            };
            if scc.scc_index < 1 || scc.scc_index > scc.scc_length {
                continue;
            }
            match (&pair.type_object, scc.equivalence_kind) {
                (TypeObject::Complete(object), EK_COMPLETE) => {
                    complete
                        .entry(scc.hash)
                        .or_insert_with(|| (scc.scc_length, BTreeMap::new()))
                        .1
                        .insert(scc.scc_index, object.clone());
                }
                (TypeObject::Minimal(object), EK_MINIMAL) => {
                    minimal
                        .entry(scc.hash)
                        .or_insert_with(|| (scc.scc_length, BTreeMap::new()))
                        .1
                        .insert(scc.scc_index, object.clone());
                }
                _ => {}
            }
        }
        let mut paired: BTreeMap<EquivalenceHash, EquivalenceHash> = BTreeMap::new();
        for link in &reply.complete_to_minimal {
            if let (
                TypeIdentifier::StronglyConnectedComponent(from),
                TypeIdentifier::StronglyConnectedComponent(to),
            ) = (&link.type_identifier1, &link.type_identifier2)
            {
                if from.equivalence_kind == EK_COMPLETE && to.equivalence_kind == EK_MINIMAL {
                    paired.insert(from.hash, to.hash);
                }
            }
        }
        let whole = |length: i32, count: usize| i32::try_from(count).ok() == Some(length);
        let mut filed = 0;
        for (hash, (length, members)) in complete {
            if whole(length, members.len()) {
                self.insert_component(hash, members.into_values().collect());
                filed += 1;
            }
        }
        // The pair a COMPLETE answer carries names the minimal counterpart of
        // each complete component, whether or not the minimal members are in
        // the same reply: they come with a later `getTypes` for the MINIMAL
        // identifier, and the link has to be there when they do.
        for (complete_hash, minimal_hash) in &paired {
            if self.components.contains_key(complete_hash) {
                self.minimal_component_of
                    .insert(*complete_hash, *minimal_hash);
            }
        }
        for (minimal_hash, (length, members)) in minimal {
            if !whole(length, members.len()) {
                continue;
            }
            let members: Vec<MinimalTypeObject> = members.into_values().collect();
            match paired.iter().find(|(_, m)| **m == minimal_hash) {
                Some((complete_hash, _)) => {
                    self.insert_minimal_component(*complete_hash, minimal_hash, members);
                }
                None => {
                    self.minimal_components.insert(minimal_hash, members);
                    self.generation = self.generation.wrapping_add(1);
                }
            }
            filed += 1;
        }
        filed
    }

    /// Files the minimal projection of every complete TypeObject that
    /// does not have one yet, and returns how many were added.
    ///
    /// XTypes 1.3 §7.3.4.5 derives both forms from one type model, and
    /// the assignability walk of §7.2.4 is defined over the minimal
    /// form: `is_assignable` projects a complete pair down and then
    /// compares minimal objects, whose members reference the **minimal**
    /// hashes of their dependencies. A registry that holds a type's
    /// complete object and not its minimal one therefore answers the
    /// lookup the identifier names and misses the one the comparison
    /// makes — the pair comes out `No("unknown type objects for hash
    /// comparison")` for a type it was just given.
    ///
    /// The projection is a fixpoint and not one pass: a complete object
    /// only projects once the complete objects of its dependencies are
    /// present, so one type completed in this call can make the next
    /// projectable. The loop is bounded by the number of complete
    /// entries, which is how many rounds a chain can need at most.
    pub fn project_completes_to_minimal(&mut self) -> usize {
        let mut added = 0usize;
        for _ in 0..=self.completes.len() {
            let mut round: Vec<(EquivalenceHash, MinimalTypeObject)> = Vec::new();
            for (_, complete) in self.completes.iter() {
                let Ok(minimal) = complete.to_minimal(self) else {
                    continue;
                };
                let Ok(hash) = crate::hash::compute_minimal_hash_with(
                    &minimal,
                    self.minimal_member_trailing_octet,
                ) else {
                    continue;
                };
                if !self.minimals.contains_key(&hash) {
                    round.push((hash, minimal));
                }
            }
            if round.is_empty() {
                break;
            }
            added += round.len();
            for (hash, minimal) in round {
                self.minimals.insert(hash, minimal);
            }
        }
        if added > 0 {
            self.generation = self.generation.wrapping_add(1);
        }
        added
    }

    /// The MINIMAL hash of the type a COMPLETE hash names, if the complete
    /// object and its dependencies are here to project it from — the
    /// pair `TypeLookup_getTypes_Out::complete_to_minimal` carries
    /// (XTypes 1.3 §7.6.3.3.4.1), so that a requester that resolved an
    /// endpoint's complete identity can reach the minimal object the
    /// assignability rules compare.
    #[must_use]
    pub fn minimal_hash_of_complete(&self, hash: &EquivalenceHash) -> Option<EquivalenceHash> {
        let complete = self.completes.get(hash)?;
        let minimal = complete.to_minimal(self).ok()?;
        crate::hash::compute_minimal_hash_with(&minimal, self.minimal_member_trailing_octet).ok()
    }

    /// The first hash in `ti`'s dependency closure that this registry
    /// cannot answer, or `None` when the whole closure is present.
    ///
    /// A type is resolvable for the assignability walk of XTypes 1.3
    /// §7.2.4 only when every TypeObject the walk reaches is in the
    /// registry, and the walk reaches more than the top-level object:
    /// a union descends into its discriminator, a struct into every
    /// member type and its base, a sequence into its element. A
    /// registry that holds the union but not the bitmask its
    /// discriminator names answers `get_minimal(union)` and then fails
    /// inside `check_minimal_types`, where the miss has no way back out
    /// — [`Assignable::No`](crate::assignability::Assignable::No) is the
    /// only shape the answer has. That is the difference between *not
    /// assignable* and *not yet known*, and only the caller that can
    /// still fetch the missing object can act on it: an unresolvable
    /// **dependency** is a `getTypes`/`getTypeDependencies` round trip
    /// (§7.6.3.3.4), not a refusal.
    ///
    /// The form travels with the walk. A `MinimalTypeObject` references
    /// the minimal hashes of its dependencies and a `CompleteTypeObject`
    /// the complete ones (§7.3.4.5 derives the two forms from one model
    /// and hashes each), so a hash reached through a minimal object is
    /// looked up in the minimal map and one reached through a complete
    /// object in the complete map. Looking a complete hash up in the
    /// minimal map is the miss that is *always* a miss.
    ///
    /// Bounded by `max_nodes` visited objects; a closure larger than
    /// that reports `None` (resolvable as far as it was walked) rather
    /// than blocking the match on a limit of ours.
    #[must_use]
    pub fn first_unresolved_dependency(
        &self,
        ti: &TypeIdentifier,
        max_nodes: usize,
    ) -> Option<TypeIdentifier> {
        // A component the registry cannot resolve is a miss like a hash:
        // the identifier goes back as it is, and `getTypes` answers an
        // SCC identifier of either kind with every member (§7.6.3.3).
        let mut components: Vec<&crate::type_identifier::StronglyConnectedComponentId> = Vec::new();
        push_component_refs(ti, &mut components);
        if let Some(missing) = components
            .into_iter()
            .find(|scc| !self.holds_component(scc))
        {
            return Some(TypeIdentifier::StronglyConnectedComponent(*missing));
        }
        let mut queue: Vec<(EquivalenceHash, bool)> = Vec::new();
        push_hash_refs(ti, &mut queue);
        let mut seen: BTreeSet<(EquivalenceHash, bool)> = BTreeSet::new();
        let mut visited = 0usize;
        while let Some((hash, complete)) = queue.pop() {
            if !seen.insert((hash, complete)) {
                continue;
            }
            visited += 1;
            if visited > max_nodes {
                return None;
            }
            let to = if complete {
                match self.completes.get(&hash) {
                    Some(c) => TypeObject::Complete(c.clone()),
                    // A component member is filed under the component's
                    // hash, not its own (§7.3.4.9.2); a hash the
                    // components map knows is answerable and is not a
                    // miss.
                    None if self.components.contains_key(&hash) => continue,
                    None => return Some(TypeIdentifier::EquivalenceHashComplete(hash)),
                }
            } else {
                match self.minimals.get(&hash) {
                    Some(m) => TypeObject::Minimal(m.clone()),
                    None if self.components.contains_key(&hash) => continue,
                    None => return Some(TypeIdentifier::EquivalenceHashMinimal(hash)),
                }
            };
            let Ok(children) = collect_referenced_hashes(&to, DEFAULT_MAX_RESOLVE_DEPTH) else {
                // Depth or node limit inside one object: as above, a
                // limit of ours is not the peer's missing type.
                continue;
            };
            for child in children {
                if !seen.contains(&(child, complete)) {
                    queue.push((child, complete));
                }
            }
        }
        None
    }
}

/// Pushes every component identifier `ti` references, descending through
/// plain collections as [`push_hash_refs`] does, with the same bound on
/// the nesting.
///
/// zerodds-lint: recursion-depth 16 (plain collection nesting; bounded
/// by `TypeIdentifier::MAX_DECODE_DEPTH`, enforced in `decode_from`)
fn push_component_refs<'a>(
    ti: &'a TypeIdentifier,
    out: &mut Vec<&'a crate::type_identifier::StronglyConnectedComponentId>,
) {
    match ti {
        TypeIdentifier::StronglyConnectedComponent(scc) => out.push(scc),
        TypeIdentifier::PlainSequenceSmall { element, .. }
        | TypeIdentifier::PlainSequenceLarge { element, .. }
        | TypeIdentifier::PlainArraySmall { element, .. }
        | TypeIdentifier::PlainArrayLarge { element, .. } => push_component_refs(element, out),
        TypeIdentifier::PlainMapSmall { element, key, .. }
        | TypeIdentifier::PlainMapLarge { element, key, .. } => {
            push_component_refs(element, out);
            push_component_refs(key, out);
        }
        _ => {}
    }
}

/// Pushes every hash `ti` references, with `true` for the complete form
/// and `false` for the minimal one, descending through plain
/// collections (whose element/key identifiers carry their own form).
///
/// The descent follows the nesting of the one identifier it is given,
/// and that nesting is bounded where the identifier is built:
/// [`TypeIdentifier::decode_from`] refuses anything deeper than
/// `TypeIdentifier::MAX_DECODE_DEPTH`, which is 16, on every identifier
/// read from the wire. The one caller outside this file's tests is
/// `zerodds_dcps`'s `unresolved_type_hash`, whose writer and reader
/// identifiers come from a peer's discovery announcement and therefore
/// through that decoder.
///
/// zerodds-lint: recursion-depth 16 (plain collection nesting; bounded
/// by `TypeIdentifier::MAX_DECODE_DEPTH`, enforced in `decode_from`)
fn push_hash_refs(ti: &TypeIdentifier, out: &mut Vec<(EquivalenceHash, bool)>) {
    match ti {
        TypeIdentifier::EquivalenceHashMinimal(h) => out.push((*h, false)),
        TypeIdentifier::EquivalenceHashComplete(h) => out.push((*h, true)),
        TypeIdentifier::PlainSequenceSmall { element, .. }
        | TypeIdentifier::PlainSequenceLarge { element, .. }
        | TypeIdentifier::PlainArraySmall { element, .. }
        | TypeIdentifier::PlainArrayLarge { element, .. } => push_hash_refs(element, out),
        TypeIdentifier::PlainMapSmall { element, key, .. }
        | TypeIdentifier::PlainMapLarge { element, key, .. } => {
            push_hash_refs(element, out);
            push_hash_refs(key, out);
        }
        _ => {}
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
/// Depth/node limits and unsupported references are errors, not an empty or
/// partial dependency list. SCC identities require a richer result than hashes
/// alone and are explicitly refused by this collector.
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
        MinimalTypeObject::Annotation(a) => {
            for member in &a.member_seq {
                collect_from_ti(&member.common.member_type_id, max_depth, depth, out, seen)?;
            }
        }
        MinimalTypeObject::Enumerated(_)
        | MinimalTypeObject::Bitmask(_)
        | MinimalTypeObject::Bitset(_) => {}
        _ => return Err(ResolveError::UnsupportedReference),
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
    let mut annotations = Vec::new();
    match c {
        CompleteTypeObject::Struct(s) => {
            annotations.push(&s.header.detail.ann_custom);
            collect_from_ti(&s.header.base_type, max_depth, depth, out, seen)?;
            for mem in &s.member_seq {
                annotations.push(&mem.detail.ann_custom);
                collect_from_ti(&mem.common.member_type_id, max_depth, depth, out, seen)?;
            }
        }
        CompleteTypeObject::Union(u) => {
            annotations.push(&u.header.detail.ann_custom);
            annotations.push(&u.discriminator.ann_custom);
            collect_from_ti(&u.discriminator.common.type_id, max_depth, depth, out, seen)?;
            for mem in &u.member_seq {
                annotations.push(&mem.detail.ann_custom);
                collect_from_ti(&mem.common.type_id, max_depth, depth, out, seen)?;
            }
        }
        CompleteTypeObject::Alias(a) => {
            annotations.push(&a.header.detail.ann_custom);
            annotations.push(&a.body.ann_custom);
            collect_from_ti(&a.body.related_type, max_depth, depth, out, seen)?;
        }
        CompleteTypeObject::Sequence(s) => {
            annotations.extend(s.detail.as_ref().map(|d| &d.ann_custom));
            annotations.push(&s.element.ann_custom);
            collect_from_ti(&s.element.common.type_id, max_depth, depth, out, seen)?;
        }
        CompleteTypeObject::Array(a) => {
            annotations.push(&a.detail.ann_custom);
            annotations.push(&a.element.ann_custom);
            collect_from_ti(&a.element.common.type_id, max_depth, depth, out, seen)?;
        }
        CompleteTypeObject::Map(m) => {
            annotations.extend(m.detail.as_ref().map(|d| &d.ann_custom));
            annotations.push(&m.key.ann_custom);
            annotations.push(&m.element.ann_custom);
            collect_from_ti(&m.key.common.type_id, max_depth, depth, out, seen)?;
            collect_from_ti(&m.element.common.type_id, max_depth, depth, out, seen)?;
        }
        CompleteTypeObject::Annotation(a) => {
            // Annex B's `CompleteAnnotationHeader` carries the annotation's
            // qualified name and nothing else — no `ann_builtin`, no
            // `ann_custom` — so an annotation type has no applied
            // annotation of its own to walk. Its parameters' type
            // identifiers are the references it does have.
            for member in &a.member_seq {
                collect_from_ti(&member.common.member_type_id, max_depth, depth, out, seen)?;
            }
        }
        CompleteTypeObject::Enumerated(e) => {
            annotations.push(&e.header.detail.ann_custom);
            annotations.extend(e.literal_seq.iter().map(|v| &v.detail.ann_custom));
        }
        CompleteTypeObject::Bitmask(b) => {
            annotations.push(&b.detail.ann_custom);
            annotations.extend(b.flag_seq.iter().map(|v| &v.detail.ann_custom));
        }
        CompleteTypeObject::Bitset(b) => {
            annotations.push(&b.detail.ann_custom);
            annotations.extend(b.field_seq.iter().map(|v| &v.detail.ann_custom));
        }
        _ => return Err(ResolveError::UnsupportedReference),
    }
    for seq in annotations {
        if let Some(values) = &seq.0 {
            for annotation in values {
                collect_from_ti(&annotation.annotation_typeid, max_depth, depth, out, seen)?;
            }
        }
    }
    Ok(())
}

/// Every identifier a complete object names, with a plain collection
/// followed down to the identifiers it is made of.
///
/// What `collect_referenced_hashes` walks, without its refusal of a
/// strongly connected component: the caller sorts the identifiers it gets
/// into hashes, components and what it cannot follow. The annotations
/// applied to the object and to its members count, because an
/// `annotation_typeid` is the hash of a `TypeObject` the object depends
/// on (§7.3.4.5).
///
/// `None` for the `default:` arm of Annex B's `CompleteTypeObject`, whose
/// content this revision cannot enumerate.
///
/// zerodds-lint: recursion-depth 16 (plain collection nesting, bounded by
/// `TypeIdentifier::MAX_DECODE_DEPTH` because the input comes from the
/// wire decoder)
#[must_use]
pub fn named_identifiers(object: &CompleteTypeObject) -> Option<Vec<&TypeIdentifier>> {
    /// zerodds-lint: recursion-depth 16 (plain collection nesting, bounded by
    /// `TypeIdentifier::MAX_DECODE_DEPTH` in the decoder that built `ti`)
    fn leaves<'a>(ti: &'a TypeIdentifier, out: &mut Vec<&'a TypeIdentifier>) {
        match ti {
            TypeIdentifier::PlainSequenceSmall { element, .. }
            | TypeIdentifier::PlainSequenceLarge { element, .. }
            | TypeIdentifier::PlainArraySmall { element, .. }
            | TypeIdentifier::PlainArrayLarge { element, .. } => leaves(element, out),
            TypeIdentifier::PlainMapSmall { element, key, .. }
            | TypeIdentifier::PlainMapLarge { element, key, .. } => {
                leaves(element, out);
                leaves(key, out);
            }
            other => out.push(other),
        }
    }
    let mut roots: Vec<&TypeIdentifier> = Vec::new();
    let mut annotations: Vec<&crate::type_object::common::OptionalAppliedAnnotationSeq> =
        Vec::new();
    match object {
        CompleteTypeObject::Struct(s) => {
            annotations.push(&s.header.detail.ann_custom);
            roots.push(&s.header.base_type);
            for member in &s.member_seq {
                annotations.push(&member.detail.ann_custom);
                roots.push(&member.common.member_type_id);
            }
        }
        CompleteTypeObject::Union(u) => {
            annotations.push(&u.header.detail.ann_custom);
            annotations.push(&u.discriminator.ann_custom);
            roots.push(&u.discriminator.common.type_id);
            for member in &u.member_seq {
                annotations.push(&member.detail.ann_custom);
                roots.push(&member.common.type_id);
            }
        }
        CompleteTypeObject::Alias(a) => {
            annotations.push(&a.header.detail.ann_custom);
            annotations.push(&a.body.ann_custom);
            roots.push(&a.body.related_type);
        }
        CompleteTypeObject::Sequence(c) => {
            annotations.extend(c.detail.as_ref().map(|d| &d.ann_custom));
            annotations.push(&c.element.ann_custom);
            roots.push(&c.element.common.type_id);
        }
        CompleteTypeObject::Array(c) => {
            annotations.push(&c.detail.ann_custom);
            annotations.push(&c.element.ann_custom);
            roots.push(&c.element.common.type_id);
        }
        CompleteTypeObject::Map(c) => {
            annotations.extend(c.detail.as_ref().map(|d| &d.ann_custom));
            annotations.push(&c.key.ann_custom);
            annotations.push(&c.element.ann_custom);
            roots.push(&c.key.common.type_id);
            roots.push(&c.element.common.type_id);
        }
        CompleteTypeObject::Annotation(a) => {
            roots.extend(a.member_seq.iter().map(|m| &m.common.member_type_id));
        }
        CompleteTypeObject::Enumerated(e) => {
            annotations.push(&e.header.detail.ann_custom);
            annotations.extend(e.literal_seq.iter().map(|v| &v.detail.ann_custom));
        }
        CompleteTypeObject::Bitmask(b) => {
            annotations.push(&b.detail.ann_custom);
            annotations.extend(b.flag_seq.iter().map(|v| &v.detail.ann_custom));
        }
        CompleteTypeObject::Bitset(b) => {
            annotations.push(&b.detail.ann_custom);
            annotations.extend(b.field_seq.iter().map(|v| &v.detail.ann_custom));
        }
        _ => return None,
    }
    let mut out = Vec::new();
    for root in roots {
        leaves(root, &mut out);
    }
    for sequence in annotations {
        if let Some(applied) = &sequence.0 {
            out.extend(applied.iter().map(|a| &a.annotation_typeid));
        }
    }
    Some(out)
}

/// zerodds-lint: recursion-depth 16
///
/// Walks plain collection elements recursively (Sequence→Map→Array nesting).
/// The depth is effectively bounded by `TypeIdentifier::MAX_DECODE_DEPTH` (=16),
/// because the input comes from the wire decoder.
fn collect_from_ti(
    ti: &TypeIdentifier,
    max_depth: usize,
    depth: usize,
    out: &mut Vec<EquivalenceHash>,
    seen: &mut Vec<EquivalenceHash>,
) -> Result<(), ResolveError> {
    if depth >= max_depth {
        return Err(ResolveError::DepthExceeded { limit: max_depth });
    }
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
            collect_from_ti(element, max_depth, depth + 1, out, seen)?;
        }
        TypeIdentifier::PlainMapSmall { element, key, .. }
        | TypeIdentifier::PlainMapLarge { element, key, .. } => {
            collect_from_ti(element, max_depth, depth + 1, out, seen)?;
            collect_from_ti(key, max_depth, depth + 1, out, seen)?;
        }
        TypeIdentifier::None
        | TypeIdentifier::Primitive(_)
        | TypeIdentifier::String8Small { .. }
        | TypeIdentifier::String8Large { .. }
        | TypeIdentifier::String16Small { .. }
        | TypeIdentifier::String16Large { .. } => {}
        TypeIdentifier::StronglyConnectedComponent(_) => {
            return Err(ResolveError::UnsupportedComponentReference);
        }
        _ => return Err(ResolveError::UnsupportedReference),
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
    fn collector_keeps_custom_annotation_references_and_refuses_unknown_objects() {
        use crate::type_object::common::{AppliedAnnotation, OptionalAppliedAnnotationSeq};
        let hash = EquivalenceHash([0x41; 14]);
        let mut object = TypeObjectBuilder::struct_type("Annotated").build_complete();
        object.header.detail.ann_custom =
            OptionalAppliedAnnotationSeq(Some(alloc::vec![AppliedAnnotation {
                annotation_typeid: TypeIdentifier::EquivalenceHashComplete(hash),
                param_seq: alloc::vec![],
            }]));
        assert_eq!(
            collect_referenced_hashes(
                &TypeObject::Complete(CompleteTypeObject::Struct(object)),
                DEFAULT_MAX_RESOLVE_DEPTH
            ),
            Ok(alloc::vec![hash])
        );
        assert_eq!(
            collect_referenced_hashes(
                &TypeObject::Minimal(MinimalTypeObject::Extended {
                    type_kind: 0x7e,
                    body: alloc::vec![]
                }),
                DEFAULT_MAX_RESOLVE_DEPTH
            ),
            Err(ResolveError::UnsupportedReference)
        );
    }

    #[test]
    fn collector_keeps_annotation_parameter_type_references() {
        use crate::type_object::common::{AnnotationParameterValue, NameHash};
        use crate::type_object::minimal::{
            CommonAnnotationParameter, MinimalAnnotationParameter, MinimalAnnotationType,
        };
        let hash = EquivalenceHash([0x42; 14]);
        let annotation = MinimalAnnotationType {
            annotation_flag: Default::default(),
            member_seq: alloc::vec![MinimalAnnotationParameter {
                common: CommonAnnotationParameter {
                    member_flags: Default::default(),
                    member_type_id: TypeIdentifier::EquivalenceHashMinimal(hash)
                },
                name_hash: NameHash::from_name("value"),
                default_value: AnnotationParameterValue::Int32(0),
            }],
        };
        assert_eq!(
            collect_referenced_hashes(
                &TypeObject::Minimal(MinimalTypeObject::Annotation(annotation)),
                DEFAULT_MAX_RESOLVE_DEPTH
            ),
            Ok(alloc::vec![hash])
        );
    }

    #[test]
    fn collector_counts_plain_collection_depth_and_keeps_the_boundary_hash() {
        use crate::type_identifier::PlainCollectionHeader;
        let hash = EquivalenceHash([0x31; 14]);
        let mut ti = TypeIdentifier::EquivalenceHashMinimal(hash);
        for _ in 0..3 {
            ti = TypeIdentifier::PlainSequenceSmall {
                header: PlainCollectionHeader::default(),
                bound: 1,
                element: alloc::boxed::Box::new(ti),
            };
        }
        let object = TypeObject::Minimal(MinimalTypeObject::Alias(make_alias(ti)));
        assert_eq!(collect_referenced_hashes(&object, 5), Ok(alloc::vec![hash]));
        assert_eq!(
            collect_referenced_hashes(&object, 4),
            Err(ResolveError::DepthExceeded { limit: 4 })
        );
    }

    #[test]
    fn collector_refuses_scc_instead_of_reporting_no_dependencies() {
        use crate::type_identifier::StronglyConnectedComponentId;
        let scc = TypeIdentifier::StronglyConnectedComponent(StronglyConnectedComponentId {
            equivalence_kind: crate::type_identifier::kinds::EK_MINIMAL,
            hash: EquivalenceHash([0x32; 14]),
            scc_length: 1,
            scc_index: 1,
        });
        let object = TypeObject::Minimal(MinimalTypeObject::Alias(make_alias(scc)));
        assert_eq!(
            collect_referenced_hashes(&object, DEFAULT_MAX_RESOLVE_DEPTH),
            Err(ResolveError::UnsupportedComponentReference)
        );
        let plain = TypeObject::Minimal(MinimalTypeObject::Alias(make_alias(
            TypeIdentifier::Primitive(PrimitiveKind::Int32),
        )));
        assert_eq!(
            collect_referenced_hashes(&plain, DEFAULT_MAX_RESOLVE_DEPTH),
            Ok(alloc::vec![])
        );
    }

    #[test]
    fn collector_refuses_extended_identifiers_in_plain_map_keys() {
        use crate::type_identifier::{CollectionElementFlag, PlainCollectionHeader};
        let map = TypeIdentifier::PlainMapSmall {
            header: PlainCollectionHeader::default(),
            bound: 1,
            element: alloc::boxed::Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
            key_flags: CollectionElementFlag::default(),
            key: alloc::boxed::Box::new(TypeIdentifier::Extended {
                discriminator: 0x7e,
                body: alloc::vec![],
            }),
        };
        let object = TypeObject::Minimal(MinimalTypeObject::Alias(make_alias(map)));
        assert_eq!(
            collect_referenced_hashes(&object, DEFAULT_MAX_RESOLVE_DEPTH),
            Err(ResolveError::UnsupportedReference)
        );
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
        // Plain TypeIdentifier in member type: nested Sequence<of Map<of Hash>>
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
    fn first_unresolved_dependency_names_the_missing_member_type() {
        // Registry holds the aggregate and not what its member names —
        // the shape the assignability walk cannot answer and used to
        // report as `No("unknown type objects for hash comparison")`.
        let h_outer = EquivalenceHash([0x40; 14]);
        let h_inner = EquivalenceHash([0x41; 14]);
        let mut reg = TypeRegistry::new();
        reg.insert_minimal(h_outer, struct_with_member_ref("Outer", h_inner));

        assert_eq!(
            reg.first_unresolved_dependency(
                &TypeIdentifier::EquivalenceHashMinimal(h_outer),
                DEFAULT_MAX_RESOLVE_NODES,
            ),
            Some(TypeIdentifier::EquivalenceHashMinimal(h_inner)),
        );

        // With the member type registered, the whole closure resolves.
        reg.insert_minimal(
            h_inner,
            MinimalTypeObject::Struct(
                TypeObjectBuilder::struct_type("Inner")
                    .member("p", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                    .build_minimal(),
            ),
        );
        assert_eq!(
            reg.first_unresolved_dependency(
                &TypeIdentifier::EquivalenceHashMinimal(h_outer),
                DEFAULT_MAX_RESOLVE_NODES,
            ),
            None,
        );
    }

    #[test]
    fn first_unresolved_dependency_keeps_the_hash_form() {
        // The two forms of one type have different hashes (§7.3.4.5), so
        // a complete identifier resolved against the minimal map is a
        // miss that no reply can ever fill. A minimal entry filed under
        // the complete hash must therefore NOT count as an answer.
        let h = EquivalenceHash([0x50; 14]);
        let mut reg = TypeRegistry::new();
        reg.insert_minimal(
            h,
            MinimalTypeObject::Struct(
                TypeObjectBuilder::struct_type("OnlyMinimal")
                    .member("p", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                    .build_minimal(),
            ),
        );
        assert_eq!(
            reg.first_unresolved_dependency(
                &TypeIdentifier::EquivalenceHashMinimal(h),
                DEFAULT_MAX_RESOLVE_NODES,
            ),
            None,
        );
        assert_eq!(
            reg.first_unresolved_dependency(
                &TypeIdentifier::EquivalenceHashComplete(h),
                DEFAULT_MAX_RESOLVE_NODES,
            ),
            Some(TypeIdentifier::EquivalenceHashComplete(h)),
        );
    }

    #[test]
    fn first_unresolved_dependency_walks_through_a_plain_collection() {
        // The element of a plain sequence is a dependency like any
        // other: the identifier carries it, and the registry has to
        // answer for it before the walk can compare anything.
        let h_elem = EquivalenceHash([0x60; 14]);
        let reg = TypeRegistry::new();
        let seq = TypeIdentifier::PlainSequenceSmall {
            header: crate::type_identifier::PlainCollectionHeader::for_element(
                crate::type_identifier::kinds::EK_MINIMAL,
            ),
            bound: 10,
            element: alloc::boxed::Box::new(TypeIdentifier::EquivalenceHashMinimal(h_elem)),
        };
        assert_eq!(
            reg.first_unresolved_dependency(&seq, DEFAULT_MAX_RESOLVE_NODES),
            Some(TypeIdentifier::EquivalenceHashMinimal(h_elem)),
        );
    }

    #[test]
    fn first_unresolved_dependency_terminates_on_a_cycle() {
        // A -> B -> A, both present: a closure walk that did not mark
        // visited nodes would not come back.
        let h_a = EquivalenceHash([0x70; 14]);
        let h_b = EquivalenceHash([0x71; 14]);
        let mut reg = TypeRegistry::new();
        reg.insert_minimal(h_a, struct_with_member_ref("A", h_b));
        reg.insert_minimal(h_b, struct_with_member_ref("B", h_a));
        assert_eq!(
            reg.first_unresolved_dependency(
                &TypeIdentifier::EquivalenceHashMinimal(h_a),
                DEFAULT_MAX_RESOLVE_NODES,
            ),
            None,
        );
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
