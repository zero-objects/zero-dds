// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Stable node identity. Spec §1.2.

use crate::error::AstCoreError;

/// Identity namespace.
///
/// Keeps programmatically synthesised nodes and built-in definitions
/// from colliding with parsed model content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IdSpace(u32);

impl IdSpace {
    /// The number this space is, for a consumer that has to name it —
    /// an adapter presenting the model to another graph, for one.
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }

    /// Nodes produced by a frontend from source material.
    pub const MODEL: Self = Self(0);
    /// Built-in definitions the model always carries: the builtin
    /// module and the built-in annotations.
    pub const BUILTIN: Self = Self(1);
    /// Nodes synthesised programmatically with no source origin.
    pub const SYNTHETIC: Self = Self(2);

    /// Every namespace, for exhaustive iteration by the snapshot codec.
    pub const ALL: [Self; 3] = [Self::MODEL, Self::BUILTIN, Self::SYNTHETIC];

    /// Raw namespace number.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// Construct a namespace from its raw number.
    #[must_use]
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }
}

/// Stable node identity: a namespace and a monotonic counter, packed
/// into 128 bits.
///
/// Survives snapshot round-trips and crosses process boundaries, unlike
/// [`crate::Id`], which is only an arena position. Spec §1.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(u128);

impl NodeId {
    /// Compose from namespace and counter.
    #[must_use]
    pub const fn from_parts(space: IdSpace, counter: u64) -> Self {
        Self(((space.0 as u128) << 64) | (counter as u128))
    }

    /// Namespace component.
    #[must_use]
    pub const fn space(self) -> IdSpace {
        IdSpace((self.0 >> 64) as u32)
    }

    /// Counter component.
    #[must_use]
    pub const fn counter(self) -> u64 {
        self.0 as u64
    }

    /// Big-endian byte form, used by the snapshot codec so that the file
    /// layout does not depend on host endianness. Spec §9.
    #[must_use]
    pub const fn to_bytes(self) -> [u8; 16] {
        self.0.to_be_bytes()
    }

    /// Inverse of [`NodeId::to_bytes`].
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(u128::from_be_bytes(bytes))
    }
}

/// Deterministic identity allocator.
///
/// The same sequence of allocations always yields the same identifiers,
/// which is what lets a snapshot hash key a build cache. Spec §9.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdAllocator {
    space: IdSpace,
    next: u64,
}

impl IdAllocator {
    /// Allocator starting at counter 1 in the given namespace.
    ///
    /// Counter 0 is reserved so that a zero identity is always invalid.
    #[must_use]
    pub const fn new(space: IdSpace) -> Self {
        Self { space, next: 1 }
    }

    /// The namespace this allocator mints into.
    #[must_use]
    pub const fn space(&self) -> IdSpace {
        self.space
    }

    /// The counter the next allocation will use. The snapshot codec
    /// writes this so a loaded model resumes exactly where it left off.
    #[must_use]
    pub const fn next_counter(&self) -> u64 {
        self.next
    }

    /// Allocate the next identity.
    ///
    /// Not named `next`: that would shadow `Iterator::next` at every
    /// call site for a type that is not an iterator.
    ///
    /// # Errors
    /// [`AstCoreError::Exhausted`] when the counter space is used up.
    pub fn allocate(&mut self) -> Result<NodeId, AstCoreError> {
        let counter = self.next;
        self.next = self.next.checked_add(1).ok_or(AstCoreError::Exhausted)?;
        Ok(NodeId::from_parts(self.space, counter))
    }

    /// Continue allocation past an identity loaded from a snapshot, so
    /// that reloading a model and continuing to edit it cannot mint a
    /// duplicate. Identities from a different namespace are ignored.
    pub fn resume_after(&mut self, highest: NodeId) {
        if highest.space() == self.space {
            self.next = self.next.max(highest.counter().saturating_add(1));
        }
    }

    /// Set the next counter directly, for snapshot loading.
    pub fn set_next_counter(&mut self, counter: u64) {
        self.next = self.next.max(counter);
    }
}

#[cfg(test)]
mod tests {
    // Tests assert on Results directly; a failed expect IS the test failure.
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn allocation_is_deterministic_for_the_same_sequence() {
        let mut a = IdAllocator::new(IdSpace::MODEL);
        let mut b = IdAllocator::new(IdSpace::MODEL);
        for _ in 0..1000 {
            assert_eq!(a.allocate(), b.allocate());
        }
    }

    #[test]
    fn spaces_do_not_collide() {
        let mut model = IdAllocator::new(IdSpace::MODEL);
        let mut builtin = IdAllocator::new(IdSpace::BUILTIN);
        let m = model.allocate().expect("model id");
        let b = builtin.allocate().expect("builtin id");
        assert_ne!(m, b);
        assert_eq!(m.counter(), b.counter());
        assert_ne!(m.space(), b.space());
    }

    #[test]
    fn byte_round_trip_is_exact() {
        let mut alloc = IdAllocator::new(IdSpace::MODEL);
        let id = alloc.allocate().expect("id");
        assert_eq!(NodeId::from_bytes(id.to_bytes()), id);
    }

    #[test]
    fn resume_after_continues_past_a_loaded_snapshot() {
        let mut first = IdAllocator::new(IdSpace::MODEL);
        let a = first.allocate().expect("a");
        let b = first.allocate().expect("b");

        let mut resumed = IdAllocator::new(IdSpace::MODEL);
        resumed.resume_after(b);
        let c = resumed.allocate().expect("c");

        assert!(c > b && b > a);
    }

    #[test]
    fn resume_after_ignores_a_foreign_namespace() {
        let mut alloc = IdAllocator::new(IdSpace::MODEL);
        alloc.resume_after(NodeId::from_parts(IdSpace::BUILTIN, 9999));
        assert_eq!(alloc.allocate().expect("id").counter(), 1);
    }

    #[test]
    fn ordering_follows_the_counter_within_a_space() {
        let mut alloc = IdAllocator::new(IdSpace::MODEL);
        let a = alloc.allocate().expect("a");
        let b = alloc.allocate().expect("b");
        assert!(a < b);
    }

    #[test]
    fn counter_zero_is_never_allocated() {
        let mut alloc = IdAllocator::new(IdSpace::MODEL);
        assert_eq!(alloc.allocate().expect("id").counter(), 1);
    }
}
