// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The samples of a topic with a DataReader (`out`, `bidir`) that wait for
//! the broker, as the topic's `qos.history` keeps them — Spec §6.
//!
//! The pump of a topic publishes one sample at a time; while a QoS 1/2
//! PUBLISH waits for a free slot under the broker's Receive Maximum, the
//! DDS samples that arrive wait here. `keep_last` with depth N keeps the
//! newest N of each instance (an instance is a key hash; the samples without
//! one are one instance) and drops the oldest of that instance; `keep_all`
//! keeps every sample. Samples leave in the order they arrived.

use std::collections::{BTreeMap, VecDeque};

/// The instance of a sample: its key hash, `None` for the samples without
/// one.
pub type Instance = Option<[u8; 16]>;

/// The waiting samples of one topic.
#[derive(Debug)]
pub struct HistoryQueue<T> {
    /// The N of `keep_last`; `usize::MAX` under `keep_all`.
    depth: usize,
    items: VecDeque<(Instance, T)>,
    /// How many samples of each instance wait.
    per_instance: BTreeMap<Instance, usize>,
    /// Samples `keep_last` dropped since the start.
    dropped: u64,
}

impl<T> HistoryQueue<T> {
    /// An empty queue that keeps `depth` samples of each instance
    /// ([`super::config::TopicConfig::history_depth`]: `usize::MAX` keeps
    /// every one). A depth of 0 is taken as 1.
    #[must_use]
    pub fn new(depth: usize) -> Self {
        Self {
            depth: depth.max(1),
            items: VecDeque::new(),
            per_instance: BTreeMap::new(),
            dropped: 0,
        }
    }

    /// Whether `instance` holds its N samples already.
    fn full(&self, instance: &Instance) -> bool {
        self.per_instance.get(instance).copied().unwrap_or(0) >= self.depth
    }

    fn count(&mut self, instance: Instance, delta: isize) {
        let n = self.per_instance.entry(instance).or_insert(0);
        *n = n.saturating_add_signed(delta);
        if *n == 0 {
            self.per_instance.remove(&instance);
        }
    }

    /// Adds the newest sample of `instance`. Under `keep_last` with its N
    /// samples waiting, the oldest of them is dropped first; the return
    /// value says whether one was.
    pub fn push(&mut self, instance: Instance, item: T) -> bool {
        let mut dropped = false;
        if self.full(&instance) {
            if let Some(pos) = self.items.iter().position(|(i, _)| *i == instance) {
                self.items.remove(pos);
                self.count(instance, -1);
                self.dropped += 1;
                dropped = true;
            }
        }
        self.items.push_back((instance, item));
        self.count(instance, 1);
        dropped
    }

    /// The oldest waiting sample, taken out.
    pub fn pop(&mut self) -> Option<(Instance, T)> {
        let (instance, item) = self.items.pop_front()?;
        self.count(instance, -1);
        Some((instance, item))
    }

    /// Puts back a sample [`Self::pop`] took whose PUBLISH could not go out
    /// yet; it is again the oldest. When its instance has received its N
    /// newer samples meanwhile, `keep_last` drops it instead; the return
    /// value says whether it did.
    pub fn put_back(&mut self, instance: Instance, item: T) -> bool {
        if self.full(&instance) {
            self.dropped += 1;
            return true;
        }
        self.items.push_front((instance, item));
        self.count(instance, 1);
        false
    }

    /// How many samples wait.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether no sample waits.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The samples `keep_last` dropped since the start.
    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.dropped
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::vec::Vec;

    const A: Instance = Some([0xa; 16]);
    const B: Instance = Some([0xb; 16]);

    fn drain(q: &mut HistoryQueue<u32>) -> Vec<u32> {
        core::iter::from_fn(|| q.pop().map(|(_, v)| v)).collect()
    }

    /// Spec §6 KEEP_LAST(N): the newest N samples of each instance wait;
    /// the oldest of an instance is dropped and counted, the others stay in
    /// the order they came.
    #[test]
    fn mqttbridge_6_keep_last_keeps_the_newest_n_of_each_instance() {
        let mut q = HistoryQueue::new(2);
        for (i, v) in [(A, 1), (B, 2), (A, 3), (A, 4), (B, 5), (A, 6)] {
            q.push(i, v);
        }
        assert_eq!(q.dropped(), 2, "A 1 and A 3");
        assert_eq!(drain(&mut q), [2, 4, 5, 6]);
        let mut keyless = HistoryQueue::new(1);
        assert!(!keyless.push(None, 1));
        assert!(keyless.push(None, 2));
        assert_eq!(drain(&mut keyless), [2]);
    }

    /// Spec §6 KEEP_ALL: no sample is dropped.
    #[test]
    fn mqttbridge_6_keep_all_drops_nothing() {
        let mut q = HistoryQueue::new(usize::MAX);
        for v in 0..1000 {
            assert!(!q.push(A, v));
        }
        assert_eq!(q.len(), 1000);
        assert_eq!(q.dropped(), 0);
        assert_eq!(drain(&mut q), (0..1000).collect::<Vec<_>>());
    }

    /// A sample whose PUBLISH waits for a slot goes back to the head; when
    /// its instance received N newer ones meanwhile, KEEP_LAST drops it.
    #[test]
    fn mqttbridge_6_a_sample_put_back_is_the_oldest_or_dropped() {
        let mut q = HistoryQueue::new(2);
        q.push(A, 1);
        q.push(B, 2);
        let (i, v) = q.pop().unwrap();
        assert!(!q.put_back(i, v));
        assert_eq!(drain(&mut q), [1, 2]);

        q.push(A, 1);
        let (i, v) = q.pop().unwrap();
        q.push(A, 2);
        q.push(A, 3);
        assert!(q.put_back(i, v), "A has its two newer samples");
        assert_eq!(drain(&mut q), [2, 3]);
        assert_eq!(q.dropped(), 1);
    }
}
