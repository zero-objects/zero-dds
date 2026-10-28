// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A map as XTypes 1.3 §7.5.1.3.6 binds it where a language has no mapping of its own: "the
//! language binding for map types shall be based on the equivalent IDL definition given in
//! 7.4.1.1.4", `sequence<MapEntry<K, V>>` of a structure with a member `key` and a member
//! `value`.
//!
//! [`MapEntry`] is that structure, and a `Vec<MapEntry<K, V>>` is the sequence. [`WireMap`] is
//! what the writer takes of a map: the entries of a `BTreeMap` (ascending by key), of such a
//! `Vec` in the order it holds them, or of [`AscendingKey`] over it, which is the order
//! `zerodds_ast_layout::MapEntryOrder::AscendingKey` names.

use alloc::vec::Vec;

/// `struct MapEntry { K key; V value; }` (XTypes 1.3 §7.4.1.1.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct MapEntry<K, V> {
    /// The key.
    pub key: K,
    /// The value.
    pub value: V,
}

impl<K, V> MapEntry<K, V> {
    /// An entry.
    #[must_use]
    pub const fn new(key: K, value: V) -> Self {
        Self { key, value }
    }
}

/// What the writer takes of a map: its size and its entries in the order they go on the wire.
pub trait WireMap<K, V> {
    /// The number of entries.
    fn entry_count(&self) -> usize;

    /// Calls `visit` with each entry, in wire order, stopping at the first error.
    ///
    /// # Errors
    /// The first error `visit` returns.
    fn try_visit<E, F: FnMut(&K, &V) -> Result<(), E>>(&self, visit: F) -> Result<(), E>;
}

impl<K, V> WireMap<K, V> for alloc::collections::BTreeMap<K, V> {
    fn entry_count(&self) -> usize {
        self.len()
    }

    fn try_visit<E, F: FnMut(&K, &V) -> Result<(), E>>(&self, mut visit: F) -> Result<(), E> {
        for (key, value) in self {
            visit(key, value)?;
        }
        Ok(())
    }
}

impl<K, V> WireMap<K, V> for [MapEntry<K, V>] {
    fn entry_count(&self) -> usize {
        self.len()
    }

    fn try_visit<E, F: FnMut(&K, &V) -> Result<(), E>>(&self, mut visit: F) -> Result<(), E> {
        for entry in self {
            visit(&entry.key, &entry.value)?;
        }
        Ok(())
    }
}

impl<K, V> WireMap<K, V> for Vec<MapEntry<K, V>> {
    fn entry_count(&self) -> usize {
        self.len()
    }

    fn try_visit<E, F: FnMut(&K, &V) -> Result<(), E>>(&self, visit: F) -> Result<(), E> {
        self.as_slice().try_visit(visit)
    }
}

/// The entries of a slice in ascending order of their keys (a stable sort, so equal keys keep
/// the order they were held in): `MapEntryOrder::AscendingKey`.
#[derive(Debug, Clone, Copy)]
pub struct AscendingKey<'a, K, V>(pub &'a [MapEntry<K, V>]);

impl<K: Ord, V> WireMap<K, V> for AscendingKey<'_, K, V> {
    fn entry_count(&self) -> usize {
        self.0.len()
    }

    fn try_visit<E, F: FnMut(&K, &V) -> Result<(), E>>(&self, mut visit: F) -> Result<(), E> {
        let mut order: Vec<&MapEntry<K, V>> = self.0.iter().collect();
        order.sort_by(|left, right| left.key.cmp(&right.key));
        for entry in order {
            visit(&entry.key, &entry.value)?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn a_vec_is_visited_in_the_order_it_holds_and_ascending_key_sorts_a_copy() {
        let held = vec![
            MapEntry::new(3, 'c'),
            MapEntry::new(1, 'a'),
            MapEntry::new(2, 'b'),
        ];
        let mut seen = Vec::new();
        held.try_visit::<(), _>(|key, _| {
            seen.push(*key);
            Ok(())
        })
        .unwrap();
        assert_eq!(seen, [3, 1, 2]);
        let mut sorted = Vec::new();
        AscendingKey(&held)
            .try_visit::<(), _>(|key, _| {
                sorted.push(*key);
                Ok(())
            })
            .unwrap();
        assert_eq!(sorted, [1, 2, 3]);
        assert_eq!(held[0].key, 3, "the held entries are not reordered");
    }
}
