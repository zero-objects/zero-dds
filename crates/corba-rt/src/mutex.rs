// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `RTCORBA::Mutex` with priority inheritance (RT-CORBA §5.6).
//!
//! Two layers:
//! - [`PriorityInheritance`] — the **protocol core** (`no_std`): computes the
//!   effective (inherited) priority of a lock owner from its base priority
//!   and the priorities of the currently blocked waiters. Prevents
//!   priority inversion: a low-priority owner inherits the priority of the
//!   highest-priority waiter until it releases.
//! - [`RtMutex`] — a real, blocking mutex (`std` feature) built on
//!   `std::sync::Mutex` + `Condvar`. It grants the lock **in priority order**
//!   (highest waiter first) and makes the inherited owner priority queryable.
//!   Fully **event-driven** (Condvar `wait`), no busy-poll, no
//!   `unsafe` (the inner `std::sync::Mutex` protects the data).

use alloc::vec::Vec;

use crate::priority::Priority;

/// Priority-inheritance protocol core (RT-CORBA §5.6).
///
/// Holds the base priority of a lock owner plus the priorities of all waiters
/// currently blocked on this lock. [`effective`](Self::effective) is the
/// maximum — the priority at which the owner should run while it holds the lock.
#[derive(Debug, Clone)]
pub struct PriorityInheritance {
    base: Priority,
    waiters: Vec<Priority>,
}

impl PriorityInheritance {
    /// New state for an owner with the given base priority, without waiters.
    #[must_use]
    pub fn new(base: Priority) -> Self {
        Self {
            base,
            waiters: Vec::new(),
        }
    }

    /// The owner's base priority.
    #[must_use]
    pub fn base(&self) -> Priority {
        self.base
    }

    /// Effective priority: `max(base, highest waiter priority)`.
    #[must_use]
    pub fn effective(&self) -> Priority {
        self.waiters
            .iter()
            .copied()
            .chain(core::iter::once(self.base))
            .max()
            .unwrap_or(self.base)
    }

    /// A waiter now blocks on the lock. Returns the (possibly raised)
    /// effective priority.
    pub fn on_block(&mut self, waiter: Priority) -> Priority {
        self.waiters.push(waiter);
        self.effective()
    }

    /// A waiter was served/cancelled — its priority drops out of the
    /// inheritance. Returns the new effective priority.
    pub fn on_unblock(&mut self, waiter: Priority) -> Priority {
        if let Some(pos) = self.waiters.iter().position(|&w| w == waiter) {
            self.waiters.remove(pos);
        }
        self.effective()
    }

    /// Number of currently blocked waiters.
    #[must_use]
    pub fn waiter_count(&self) -> usize {
        self.waiters.len()
    }
}

#[cfg(feature = "std")]
pub use std_impl::{RtMutex, RtMutexGuard};

#[cfg(feature = "std")]
#[allow(clippy::expect_used)]
mod std_impl {
    use super::Priority;
    use alloc::vec::Vec;
    use std::sync::{Condvar, Mutex, MutexGuard};

    /// PI bookkeeping of an [`RtMutex`] — separate from the payload lock.
    #[derive(Debug)]
    struct Coord {
        locked: bool,
        holder: Option<Priority>,
        waiters: Vec<Priority>,
    }

    impl Coord {
        /// Highest waiting priority (if any).
        fn top_waiter(&self) -> Option<Priority> {
            self.waiters.iter().copied().max()
        }
    }

    /// A blocking mutex with priority inheritance (RT-CORBA §5.6).
    ///
    /// The lock is granted **in priority order**: when the owner releases, it
    /// goes to the highest-priority waiting thread (not FIFO). The effective
    /// owner priority (`base` raised to the highest waiter) is queryable via
    /// [`effective_holder_priority`](RtMutex::effective_holder_priority)
    /// — a runtime integration can use it to track the OS priority of the owner
    /// thread.
    #[derive(Debug)]
    pub struct RtMutex<T> {
        data: Mutex<T>,
        coord: Mutex<Coord>,
        cv: Condvar,
    }

    /// RAII guard of an [`RtMutex`]; releases the lock priority-correctly on drop.
    #[derive(Debug)]
    pub struct RtMutexGuard<'a, T> {
        mutex: &'a RtMutex<T>,
        data: Option<MutexGuard<'a, T>>,
    }

    impl<T> RtMutex<T> {
        /// New, unlocked `RtMutex` around `value`.
        #[must_use]
        pub fn new(value: T) -> Self {
            Self {
                data: Mutex::new(value),
                coord: Mutex::new(Coord {
                    locked: false,
                    holder: None,
                    waiters: Vec::new(),
                }),
                cv: Condvar::new(),
            }
        }

        /// Locks the mutex for a caller with priority `my_priority`.
        /// Blocks in priority order (Condvar, no spin) until it is its
        /// turn.
        ///
        /// # Panics
        /// On a poisoned internal lock.
        #[allow(clippy::missing_panics_doc)]
        pub fn lock(&self, my_priority: Priority) -> RtMutexGuard<'_, T> {
            {
                let mut coord = self.coord.lock().expect("rt-mutex coord poisoned");
                // ALWAYS enqueue, even when the lock is momentarily free: a fresh
                // arrival must never barge ahead of an already-waiting,
                // higher-priority thread. Taking a `!locked` fast path here would
                // let a low-priority caller grab the lock in the handoff window
                // after `unlock()` clears `locked` but before the woken
                // highest-priority waiter has re-acquired `coord` — a
                // priority-inversion bug. The wait-loop is a no-op (no condvar
                // wait) for the uncontended case: we are the sole/highest waiter
                // and the lock is free, so the predicate holds immediately.
                coord.waiters.push(my_priority);
                loop {
                    let granted = !coord.locked && coord.top_waiter() == Some(my_priority);
                    if granted {
                        break;
                    }
                    coord = self.cv.wait(coord).expect("rt-mutex cv poisoned");
                }
                // Take the lock: remove ourselves from the waiters.
                if let Some(pos) = coord.waiters.iter().position(|&w| w == my_priority) {
                    coord.waiters.remove(pos);
                }
                coord.locked = true;
                coord.holder = Some(my_priority);
            }
            let data = self.data.lock().expect("rt-mutex data poisoned");
            RtMutexGuard {
                mutex: self,
                data: Some(data),
            }
        }

        /// Attempts to lock without blocking. `None` if already locked.
        ///
        /// # Panics
        /// On a poisoned internal lock.
        #[allow(clippy::missing_panics_doc)]
        pub fn try_lock(&self, my_priority: Priority) -> Option<RtMutexGuard<'_, T>> {
            {
                let mut coord = self.coord.lock().expect("rt-mutex coord poisoned");
                if coord.locked {
                    return None;
                }
                coord.locked = true;
                coord.holder = Some(my_priority);
            }
            let data = self.data.lock().expect("rt-mutex data poisoned");
            Some(RtMutexGuard {
                mutex: self,
                data: Some(data),
            })
        }

        /// Effective priority of the current owner: its base, raised to
        /// the highest waiting priority (priority inheritance). `None` if
        /// nobody currently holds the lock.
        ///
        /// # Panics
        /// On a poisoned internal lock.
        #[allow(clippy::missing_panics_doc)]
        #[must_use]
        pub fn effective_holder_priority(&self) -> Option<Priority> {
            let coord = self.coord.lock().expect("rt-mutex coord poisoned");
            coord.holder.map(|h| match coord.top_waiter() {
                Some(w) if w > h => w,
                _ => h,
            })
        }

        fn unlock(&self) {
            let mut coord = self.coord.lock().expect("rt-mutex coord poisoned");
            coord.locked = false;
            coord.holder = None;
            // Wake everyone; the highest-priority waiter wins the next lock.
            drop(coord);
            self.cv.notify_all();
        }
    }

    impl<T> RtMutexGuard<'_, T> {
        /// Read access to the protected data.
        #[must_use]
        pub fn get(&self) -> &T {
            self.data.as_ref().expect("guard active")
        }

        /// Write access to the protected data.
        pub fn get_mut(&mut self) -> &mut T {
            self.data.as_mut().expect("guard active")
        }
    }

    impl<T> core::ops::Deref for RtMutexGuard<'_, T> {
        type Target = T;
        fn deref(&self) -> &T {
            self.get()
        }
    }

    impl<T> core::ops::DerefMut for RtMutexGuard<'_, T> {
        fn deref_mut(&mut self) -> &mut T {
            self.get_mut()
        }
    }

    impl<T> Drop for RtMutexGuard<'_, T> {
        fn drop(&mut self) {
            // Release the data lock first, then free the coordination.
            self.data.take();
            self.mutex.unlock();
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn p(v: i16) -> Priority {
        Priority::new(v).unwrap()
    }

    #[test]
    fn effective_is_base_without_waiters() {
        let pi = PriorityInheritance::new(p(10));
        assert_eq!(pi.effective(), p(10));
        assert_eq!(pi.waiter_count(), 0);
    }

    #[test]
    fn owner_inherits_highest_waiter() {
        let mut pi = PriorityInheritance::new(p(10));
        assert_eq!(pi.on_block(p(50)), p(50)); // raised
        assert_eq!(pi.on_block(p(30)), p(50)); // stays at the maximum
        assert_eq!(pi.on_block(p(90)), p(90)); // higher → inherits more
        assert_eq!(pi.waiter_count(), 3);
    }

    #[test]
    fn priority_reverts_on_unblock() {
        let mut pi = PriorityInheritance::new(p(10));
        pi.on_block(p(50));
        pi.on_block(p(90));
        assert_eq!(pi.effective(), p(90));
        assert_eq!(pi.on_unblock(p(90)), p(50)); // highest gone → drops to 50
        assert_eq!(pi.on_unblock(p(50)), p(10)); // all gone → base
    }

    #[cfg(feature = "std")]
    #[test]
    fn rt_mutex_basic_lock_unlock() {
        let m = RtMutex::new(0u32);
        {
            let mut g = m.lock(p(10));
            *g += 5;
            assert_eq!(m.effective_holder_priority(), Some(p(10)));
        }
        assert_eq!(m.effective_holder_priority(), None);
        assert_eq!(*m.lock(p(1)), 5);
    }

    #[cfg(feature = "std")]
    #[test]
    fn rt_mutex_try_lock_contended() {
        let m = RtMutex::new(());
        let g = m.lock(p(10));
        assert!(m.try_lock(p(20)).is_none());
        drop(g);
        assert!(m.try_lock(p(20)).is_some());
    }

    #[cfg(feature = "std")]
    #[test]
    fn rt_mutex_grants_highest_priority_waiter_first() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let m = Arc::new(RtMutex::new(()));
        let order = Arc::new(std::sync::Mutex::new(alloc::vec::Vec::<i16>::new()));
        let next = Arc::new(AtomicU32::new(0));

        // Low-priority owner holds the lock and blocks the waiters.
        let g = m.lock(p(1));

        // Start two waiters; wait until both are provably blocked.
        let mut handles = alloc::vec::Vec::new();
        for prio in [p(40), p(80)] {
            let m = Arc::clone(&m);
            let order = Arc::clone(&order);
            let next = Arc::clone(&next);
            handles.push(std::thread::spawn(move || {
                next.fetch_add(1, Ordering::SeqCst);
                let _lg = m.lock(prio);
                order.lock().unwrap().push(prio.value());
            }));
        }
        // Both threads have started; wait until both are in coord.waiters.
        while m.effective_holder_priority() != Some(p(80)) {
            std::thread::yield_now();
        }
        // Owner releases → highest waiter (80) must be served first.
        drop(g);
        for h in handles {
            h.join().unwrap();
        }
        let seq = order.lock().unwrap().clone();
        assert_eq!(seq, alloc::vec![80, 40]);
    }
}
