// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Generational arena with typed handles. Spec §1.1.

use crate::error::AstCoreError;
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::num::NonZeroU32;

/// Typed handle into an [`Arena`].
///
/// Distinct per node kind: a handle to a member cannot be passed where a
/// handle to a type is expected, because `Id<MemberNode>` and
/// `Id<TypeNode>` are different types. The marker is `fn() -> T` so the
/// handle stays `Send + Sync` and invariance does not depend on `T`.
pub struct Id<T> {
    slot: u32,
    generation: NonZeroU32,
    marker: PhantomData<fn() -> T>,
}

impl<T> Id<T> {
    /// Arena slot this handle addresses.
    #[must_use]
    pub const fn slot(self) -> u32 {
        self.slot
    }

    /// Generation this handle was minted with.
    #[must_use]
    pub const fn generation(self) -> u32 {
        self.generation.get()
    }

    /// The handle a fresh arena produces for `slot`, at generation 1.
    ///
    /// Infallible by construction, which lets a model constructor whose
    /// first inserts cannot fail stay infallible without an `expect` —
    /// the crate's lints forbid one. Meaningless for any arena that has
    /// already seen a removal.
    #[must_use]
    pub const fn first_at(slot: u32) -> Self {
        Self {
            slot,
            generation: NonZeroU32::MIN,
            marker: PhantomData,
        }
    }

    /// Reconstruct a handle from its parts. Used by the snapshot loader,
    /// which rebuilds arenas slot by slot.
    ///
    /// # Errors
    /// [`AstCoreError::Exhausted`] when `generation` is zero, which no
    /// live slot ever carries.
    pub fn from_parts(slot: u32, generation: u32) -> Result<Self, AstCoreError> {
        let generation = NonZeroU32::new(generation).ok_or(AstCoreError::Exhausted)?;
        Ok(Self {
            slot,
            generation,
            marker: PhantomData,
        })
    }
}

// Deriving these would impose `T: Clone`, `T: PartialEq` and so on on a
// handle that holds no `T` at all, so the impls are written by hand.
impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Id<T> {}

impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.slot == other.slot && self.generation == other.generation
    }
}

impl<T> Eq for Id<T> {}

impl<T> PartialOrd for Id<T> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Id<T> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.slot
            .cmp(&other.slot)
            .then(self.generation.cmp(&other.generation))
    }
}

impl<T> core::hash::Hash for Id<T> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.slot.hash(state);
        self.generation.hash(state);
    }
}

impl<T> core::fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Id({}#{})", self.slot, self.generation)
    }
}

struct Slot<T> {
    generation: NonZeroU32,
    value: Option<T>,
}

/// Generational arena.
///
/// Removal bumps the slot generation, so a handle taken before removal
/// fails loudly instead of addressing whichever node later occupies the
/// slot. Spec §1.1.
pub struct Arena<T> {
    slots: Vec<Slot<T>>,
    free: Vec<u32>,
    live: usize,
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: core::fmt::Debug> core::fmt::Debug for Arena<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Arena")
            .field("live", &self.live)
            .field("slots", &self.slots.len())
            .finish()
    }
}

impl<T> Arena<T> {
    /// Empty arena.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
            live: 0,
        }
    }

    /// Number of live entries.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.live
    }

    /// Whether the arena holds no live entries.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.live == 0
    }

    /// Total number of slots ever allocated, live or free. The snapshot
    /// codec writes this so a loaded arena reproduces the same slot
    /// layout. Spec §9.
    #[must_use]
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// Insert a value, returning its handle.
    ///
    /// Reuses a free slot when one is available, always with the fresh
    /// generation that slot carries after its last removal.
    ///
    /// # Errors
    /// [`AstCoreError::Exhausted`] when the slot space is used up.
    pub fn insert(&mut self, value: T) -> Result<Id<T>, AstCoreError> {
        if let Some(slot) = self.free.pop() {
            let entry = self
                .slots
                .get_mut(slot as usize)
                .ok_or(AstCoreError::UnknownSlot { slot })?;
            entry.value = Some(value);
            self.live += 1;
            return Ok(Id {
                slot,
                generation: entry.generation,
                marker: PhantomData,
            });
        }
        let slot = u32::try_from(self.slots.len()).map_err(|_| AstCoreError::Exhausted)?;
        let generation = NonZeroU32::MIN;
        self.slots.push(Slot {
            generation,
            value: Some(value),
        });
        self.live += 1;
        Ok(Id {
            slot,
            generation,
            marker: PhantomData,
        })
    }

    /// Insert a value at an explicit slot with an explicit generation.
    ///
    /// Only the snapshot loader uses this; it rebuilds an arena so that
    /// handles recorded in the snapshot stay valid. Spec §9.
    ///
    /// # Errors
    /// [`AstCoreError::MalformedSnapshot`] when the slot is already
    /// occupied, which means the snapshot named the same slot twice.
    pub fn insert_at(
        &mut self,
        slot: u32,
        generation: u32,
        value: T,
    ) -> Result<Id<T>, AstCoreError> {
        let generation = NonZeroU32::new(generation).ok_or(AstCoreError::MalformedSnapshot {
            offset: 0,
            expected: "non-zero slot generation",
        })?;
        let index = slot as usize;
        while self.slots.len() <= index {
            self.slots.push(Slot {
                generation: NonZeroU32::MIN,
                value: None,
            });
            let free_slot =
                u32::try_from(self.slots.len() - 1).map_err(|_| AstCoreError::Exhausted)?;
            self.free.push(free_slot);
        }
        let entry = self
            .slots
            .get_mut(index)
            .ok_or(AstCoreError::UnknownSlot { slot })?;
        if entry.value.is_some() {
            return Err(AstCoreError::MalformedSnapshot {
                offset: 0,
                expected: "each slot named at most once",
            });
        }
        entry.generation = generation;
        entry.value = Some(value);
        self.free.retain(|candidate| *candidate != slot);
        self.live += 1;
        Ok(Id {
            slot,
            generation,
            marker: PhantomData,
        })
    }

    fn slot_of(&self, id: Id<T>) -> Result<&Slot<T>, AstCoreError> {
        let entry = self
            .slots
            .get(id.slot as usize)
            .ok_or(AstCoreError::UnknownSlot { slot: id.slot })?;
        if entry.generation != id.generation || entry.value.is_none() {
            return Err(AstCoreError::StaleHandle {
                slot: id.slot,
                expected: id.generation.get(),
                found: entry.generation.get(),
            });
        }
        Ok(entry)
    }

    /// Borrow the value behind a handle.
    ///
    /// # Errors
    /// [`AstCoreError::StaleHandle`] if the node was removed,
    /// [`AstCoreError::UnknownSlot`] if the slot was never allocated.
    pub fn get(&self, id: Id<T>) -> Result<&T, AstCoreError> {
        let entry = self.slot_of(id)?;
        entry.value.as_ref().ok_or(AstCoreError::StaleHandle {
            slot: id.slot,
            expected: id.generation.get(),
            found: entry.generation.get(),
        })
    }

    /// Mutably borrow the value behind a handle.
    ///
    /// # Errors
    /// As [`Arena::get`].
    pub fn get_mut(&mut self, id: Id<T>) -> Result<&mut T, AstCoreError> {
        self.slot_of(id)?;
        let entry = self
            .slots
            .get_mut(id.slot as usize)
            .ok_or(AstCoreError::UnknownSlot { slot: id.slot })?;
        let found = entry.generation.get();
        entry.value.as_mut().ok_or(AstCoreError::StaleHandle {
            slot: id.slot,
            expected: id.generation.get(),
            found,
        })
    }

    /// Whether the handle addresses a live node.
    #[must_use]
    pub fn contains(&self, id: Id<T>) -> bool {
        self.slot_of(id).is_ok()
    }

    /// Remove the node, returning its value and freeing the slot with a
    /// bumped generation.
    ///
    /// # Errors
    /// As [`Arena::get`], plus [`AstCoreError::Exhausted`] when the
    /// slot's generation space is used up.
    pub fn remove(&mut self, id: Id<T>) -> Result<T, AstCoreError> {
        self.slot_of(id)?;
        let entry = self
            .slots
            .get_mut(id.slot as usize)
            .ok_or(AstCoreError::UnknownSlot { slot: id.slot })?;
        let found = entry.generation.get();
        let value = entry.value.take().ok_or(AstCoreError::StaleHandle {
            slot: id.slot,
            expected: id.generation.get(),
            found,
        })?;
        entry.generation = entry
            .generation
            .checked_add(1)
            .ok_or(AstCoreError::Exhausted)?;
        self.free.push(id.slot);
        self.live -= 1;
        Ok(value)
    }

    /// Iterate live entries in ascending slot order.
    ///
    /// The order is stable across runs, which is what the snapshot codec
    /// derives its byte determinism from. Spec §9.
    pub fn iter(&self) -> impl Iterator<Item = (Id<T>, &T)> {
        self.slots.iter().enumerate().filter_map(|(index, entry)| {
            let value = entry.value.as_ref()?;
            let slot = u32::try_from(index).ok()?;
            Some((
                Id {
                    slot,
                    generation: entry.generation,
                    marker: PhantomData,
                },
                value,
            ))
        })
    }

    /// Handles of every live entry, in ascending slot order.
    pub fn ids(&self) -> impl Iterator<Item = Id<T>> + '_ {
        self.iter().map(|(id, _)| id)
    }

    /// Mutably iterate every live entry, in ascending slot order.
    ///
    /// Needed by sweeps that have to touch the whole arena — rewriting
    /// every reference to a removed node, for instance — where a
    /// handle-by-handle loop would fight the borrow checker.
    pub fn iter_mut_all(&mut self) -> impl Iterator<Item = (Id<T>, &mut T)> {
        self.slots
            .iter_mut()
            .enumerate()
            .filter_map(|(index, entry)| {
                let generation = entry.generation;
                let value = entry.value.as_mut()?;
                let slot = u32::try_from(index).ok()?;
                Some((
                    Id {
                        slot,
                        generation,
                        marker: PhantomData,
                    },
                    value,
                ))
            })
    }
}

#[cfg(test)]
mod tests {
    // Tests assert on Results directly; a failed expect IS the test failure.
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    #[test]
    fn insert_then_get_returns_the_value() {
        let mut arena: Arena<u32> = Arena::new();
        let id = arena.insert(7).expect("insert");
        assert_eq!(arena.get(id).copied(), Ok(7));
        assert_eq!(arena.len(), 1);
    }

    #[test]
    fn remove_returns_the_value_and_empties_the_arena() {
        let mut arena: Arena<u32> = Arena::new();
        let id = arena.insert(7).expect("insert");
        assert_eq!(arena.remove(id), Ok(7));
        assert!(arena.is_empty());
    }

    #[test]
    fn handle_from_before_removal_is_stale_not_reused() {
        let mut arena: Arena<u32> = Arena::new();
        let first = arena.insert(7).expect("insert");
        let _ = arena.remove(first).expect("remove");
        let second = arena.insert(9).expect("reinsert");

        // The slot is reused, the generation is not.
        assert_eq!(first.slot(), second.slot());
        assert_ne!(first.generation(), second.generation());

        // Reading through the old handle must fail, not return 9.
        assert!(matches!(
            arena.get(first),
            Err(AstCoreError::StaleHandle { .. })
        ));
        assert_eq!(arena.get(second).copied(), Ok(9));
    }

    #[test]
    fn double_remove_is_an_error_not_a_second_value() {
        let mut arena: Arena<u32> = Arena::new();
        let id = arena.insert(7).expect("insert");
        assert_eq!(arena.remove(id), Ok(7));
        assert!(arena.remove(id).is_err());
    }

    #[test]
    fn iter_visits_only_live_entries_in_slot_order() {
        let mut arena: Arena<u32> = Arena::new();
        let a = arena.insert(1).expect("a");
        let b = arena.insert(2).expect("b");
        let c = arena.insert(3).expect("c");
        let _ = arena.remove(b).expect("remove b");

        let seen: Vec<(Id<u32>, u32)> = arena.iter().map(|(id, v)| (id, *v)).collect();
        assert_eq!(seen, vec![(a, 1), (c, 3)]);
    }

    #[test]
    fn contains_tracks_liveness() {
        let mut arena: Arena<u32> = Arena::new();
        let id = arena.insert(7).expect("insert");
        assert!(arena.contains(id));
        let _ = arena.remove(id).expect("remove");
        assert!(!arena.contains(id));
    }

    #[test]
    fn insert_at_rebuilds_a_sparse_slot_layout() {
        let mut arena: Arena<u32> = Arena::new();
        let id = arena.insert_at(3, 2, 42).expect("insert_at");
        assert_eq!(id.slot(), 3);
        assert_eq!(id.generation(), 2);
        assert_eq!(arena.get(id).copied(), Ok(42));
        assert_eq!(arena.len(), 1);
        assert_eq!(arena.slot_count(), 4);
    }

    #[test]
    fn insert_at_rejects_a_slot_named_twice() {
        let mut arena: Arena<u32> = Arena::new();
        let _ = arena.insert_at(0, 1, 1).expect("first");
        assert!(matches!(
            arena.insert_at(0, 1, 2),
            Err(AstCoreError::MalformedSnapshot { .. })
        ));
    }

    proptest::proptest! {
        /// However many inserts and removes are interleaved, a handle
        /// whose node was removed never reads back a value.
        #[test]
        fn removed_handles_never_read_back(ops in proptest::collection::vec(0u8..3, 0..200)) {
            let mut arena: Arena<u32> = Arena::new();
            let mut live: Vec<Id<u32>> = Vec::new();
            let mut dead: Vec<Id<u32>> = Vec::new();
            let mut counter = 0u32;

            for op in ops {
                if op == 0 {
                    if let Ok(id) = arena.insert(counter) {
                        live.push(id);
                        counter += 1;
                    }
                } else if let Some(id) = live.pop() {
                    let _ = arena.remove(id);
                    dead.push(id);
                }
                for id in &dead {
                    proptest::prop_assert!(arena.get(*id).is_err());
                }
            }
        }
    }
}
