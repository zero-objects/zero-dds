// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! RCU cell — copy-on-write container for low-write/high-read patterns.
//!
//! Foundation primitive for a lock-free read history cache.
//! Readers access an `Arc<T>` snapshot delivered by a short mutex
//! call (microseconds); afterwards the snapshot lives reference-counted,
//! **without** any further lock touch.
//!
//! # Design goal
//!
//! Classic RCU with an `AtomicPtr` pointer swap needs `unsafe`
//! (pointer provenance, memory ordering). This implementation trades
//! microscopic performance for strictly safe code: a `Mutex<Arc<T>>`
//! guards only the reference cell, **not** the contents — the contents
//! are cloned before the lock is released.
//!
//! Performance profile:
//!
//! * **Reader**: 1× mutex acquire/release + 1× Arc clone (refcount inc).
//!   Multiple readers **serialize** briefly on the cell mutex — each
//!   holds it only for a refcount inc, so sub-microsecond.
//! * **Writer**: 1× mutex acquire + copy-on-write of `T` (= allocation
//!   of a new inner struct via closure) + Arc replace + mutex
//!   release.
//! * **Snapshot lifetime**: after a reader read, the `Arc<T>` snapshot
//!   exists independently of the cell mutex. Readers and writers no
//!   longer interfere.
//!
//! For a true `AtomicPtr` swap (lock-free on read), `arc-swap` as an
//! external crate would be the standard choice. We avoid it as long as
//! std means suffice.
//!
//! # Usage in ZeroDDS
//!
//! `zerodds_rtps::history_cache::LockFreeReadHistoryCache` (D.4 Phase C-2)
//! uses this cell as a backing store. Monitoring/tick loops can then
//! pull snapshots without taking the writer lock — the actual insert
//! path stays synchronous under the per-slot mutex from D.4 Phase B.

#[cfg(feature = "alloc")]
use alloc::sync::Arc;

#[cfg(feature = "std")]
use std::sync::Mutex;

/// RCU cell: read-only snapshot via Arc clone, write via copy-on-write.
///
/// Generic over `T`. `T` must be `Clone` — the writer path copies the
/// current state and passes a `&T` to the mutator closure, which
/// returns a new `T`.
///
/// ```
/// use zerodds_foundation::rcu::RcuCell;
///
/// let cell = RcuCell::new(0u32);
/// let r = cell.read();
/// assert_eq!(*r, 0);
/// cell.write_with(|cur| cur + 1);
/// assert_eq!(*cell.read(), 1);
/// // The old snapshot is unchanged:
/// assert_eq!(*r, 0);
/// ```
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct RcuCell<T> {
    inner: Mutex<Arc<T>>,
}

#[cfg(feature = "std")]
impl<T> RcuCell<T> {
    /// Creates a new cell with an initial value.
    #[must_use]
    pub fn new(value: T) -> Self {
        Self {
            inner: Mutex::new(Arc::new(value)),
        }
    }

    /// Returns a read snapshot. The mutex is held **only** for the
    /// `Arc::clone()` — afterwards the returned Arc is independent of
    /// the cell.
    ///
    /// # Panics
    /// Never — mutex poisoning is bypassed via `into_inner`.
    #[must_use]
    pub fn read(&self) -> Arc<T> {
        match self.inner.lock() {
            Ok(g) => Arc::clone(&g),
            // On poisoning, return the last valid state — the cell is
            // reachable read-only because the inner is structured via
            // `Arc` and therefore `Clone`.
            Err(p) => Arc::clone(&p.into_inner()),
        }
    }

    /// Updates the contents copy-on-write. The closure receives a
    /// reference to the current state and returns the new state.
    /// The writer holds the mutex only for the closure run +
    /// Arc replace.
    pub fn write_with(&self, f: impl FnOnce(&T) -> T)
    where
        T: Clone,
    {
        let mut guard = match self.inner.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let new = f(&guard);
        *guard = Arc::new(new);
    }

    /// In-place mutation on a **local copy** of the contents.
    /// Sequence: pull snapshot, clone, the closure runs a `&mut`
    /// mutator on the copy, then Arc replace.
    pub fn modify(&self, f: impl FnOnce(&mut T))
    where
        T: Clone,
    {
        self.write_with(|cur| {
            let mut new = cur.clone();
            f(&mut new);
            new
        });
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use alloc::vec::Vec;

    #[test]
    fn new_cell_returns_initial_value() {
        let cell = RcuCell::new(42u32);
        assert_eq!(*cell.read(), 42);
    }

    #[test]
    fn write_with_replaces_value() {
        let cell = RcuCell::new(1u32);
        cell.write_with(|c| c + 10);
        assert_eq!(*cell.read(), 11);
    }

    #[test]
    fn old_snapshot_is_immutable_after_write() {
        let cell = RcuCell::new(1u32);
        let snap = cell.read();
        cell.write_with(|c| c + 100);
        assert_eq!(*snap, 1, "RCU snapshot is decoupled from later writes");
        assert_eq!(*cell.read(), 101);
    }

    #[test]
    fn multiple_readers_share_arc() {
        let cell = RcuCell::new("hello".to_string());
        let a = cell.read();
        let b = cell.read();
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn modify_uses_clone_and_mutator() {
        let cell = RcuCell::new(alloc::vec![1u32, 2, 3]);
        cell.modify(|v| v.push(4));
        let r = cell.read();
        assert_eq!(*r, alloc::vec![1, 2, 3, 4]);
    }

    #[test]
    fn concurrent_readers_writers_smoke() {
        // 1 writer, 4 readers, 10ms — no deadlock, no data loss.
        use std::sync::Arc as StdArc;
        use std::thread;
        use std::time::Duration;

        let cell: StdArc<RcuCell<u64>> = StdArc::new(RcuCell::new(0));
        let stop: StdArc<std::sync::atomic::AtomicBool> =
            StdArc::new(std::sync::atomic::AtomicBool::new(false));

        let writer = {
            let cell = StdArc::clone(&cell);
            let stop = StdArc::clone(&stop);
            thread::spawn(move || {
                let mut i = 0u64;
                while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                    cell.write_with(|_| i);
                    i = i.wrapping_add(1);
                }
                i
            })
        };
        let mut readers = Vec::new();
        for _ in 0..4 {
            let cell = StdArc::clone(&cell);
            let stop = StdArc::clone(&stop);
            readers.push(thread::spawn(move || {
                let mut last = 0u64;
                while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                    let v = *cell.read();
                    // We can only guarantee that values are monotonic;
                    // a u64 wrap after 1e10 iterations isn't seen in 10ms,
                    // so strictly monotonic.
                    assert!(v >= last);
                    last = v;
                }
                last
            }));
        }
        thread::sleep(Duration::from_millis(10));
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let writer_count = writer.join().unwrap();
        for r in readers {
            let last = r.join().unwrap();
            assert!(last <= writer_count);
        }
    }

    #[test]
    fn write_with_recovers_from_poisoned_mutex() {
        // If a writer poisons the mutex with a panic, the next
        // reader/writer should still see the last valid state.
        use std::sync::Arc as StdArc;
        let cell: StdArc<RcuCell<u32>> = StdArc::new(RcuCell::new(7));
        let cell_p = StdArc::clone(&cell);
        // Panic in the writer path: closure panicked, mutex stays poisoned.
        let _ = std::thread::spawn(move || {
            cell_p.write_with(|_| panic!("intentional"));
        })
        .join();
        // read() must not panic itself + returns the last valid value
        // (7), because the closure would only publish the mutex AFTER
        // its execution.
        assert_eq!(*cell.read(), 7);
        // write_with on a poisoned mutex — recovers.
        cell.write_with(|c| c + 1);
        assert_eq!(*cell.read(), 8);
    }
}
