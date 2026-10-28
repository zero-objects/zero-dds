// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Status listeners invoked when a status changes, not when the
//! application asks.
//!
//! DDS 1.4 §2.2.4.1: a listener is called when the status it is attached
//! to changes. A `DataReader`, `DataWriter` and `Topic` compare their
//! communication statuses against the last seen value and dispatch on a
//! difference — but only inside the status getters and
//! `drive_listeners()`, so an application that set a listener and never
//! polled heard nothing but `on_data_available`.
//!
//! The runtime says when a status moved: [`DcpsRuntime::add_status_hook`]
//! runs a hook on the thread that made the change, possibly with the
//! endpoint's slot locked, so the hook must not call back into the
//! runtime. This module's hook does the one thing it may: it marks work
//! pending and wakes a thread of its own. That thread runs the delta
//! polls of every entity that registered, and those read the runtime's
//! counters as the getters always did. A burst of changes is one pass.
//!
//! The dispatcher holds each entity weakly. A reader that is dropped
//! stops being polled at the next pass, and nothing here keeps it alive.
//!
//! [`DcpsRuntime::add_status_hook`]: crate::runtime::DcpsRuntime::add_status_hook

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, Weak};

/// Bumped by every `set_listener` of any entity of this process, so a
/// cached answer to "is a data listener in this reader's chain" knows
/// when it may have gone stale.
static LISTENER_GENERATION: AtomicU64 = AtomicU64::new(0);

/// The current [`LISTENER_GENERATION`].
pub(crate) fn listener_generation() -> u64 {
    LISTENER_GENERATION.load(Ordering::Acquire)
}

/// An entity whose status listeners this dispatcher runs.
pub(crate) trait StatusDriver: Send + Sync {
    /// Compares every status against the last seen value and dispatches
    /// the ones that changed. The same polls the getters run.
    fn drive(&self);
}

/// What the dispatcher's thread has to do on its next pass. A reader's
/// data listeners are not among it: the thread that hands the reader its
/// samples calls them (`ReaderStatus::dispatch_data`).
#[derive(Default)]
struct Pending {
    /// A status changed: every poll of every entity.
    status: bool,
    /// The thread is blocked in its wait, so marking work pending has to
    /// wake it; while it runs a pass it looks at `status` before it waits
    /// again.
    waiting: bool,
}

/// One per runtime; see the module documentation.
pub(crate) struct ListenerDispatcher {
    drivers: Mutex<Vec<Weak<dyn StatusDriver>>>,
    pending: Mutex<Pending>,
    wake: Condvar,
    stop: AtomicBool,
}

impl ListenerDispatcher {
    /// A dispatcher and the thread that runs it. The thread ends after
    /// [`Self::stop`].
    pub(crate) fn start() -> Arc<Self> {
        let dispatcher = Arc::new(Self {
            drivers: Mutex::new(Vec::new()),
            pending: Mutex::new(Pending::default()),
            wake: Condvar::new(),
            stop: AtomicBool::new(false),
        });
        let runner = Arc::clone(&dispatcher);
        let spawned = std::thread::Builder::new()
            .name("zerodds-listeners".into())
            .spawn(move || runner.run());
        if spawned.is_err() {
            // No thread: the getters and `drive_listeners()` still
            // dispatch, which is what the API did before this existed.
            dispatcher.stop.store(true, Ordering::Release);
        }
        dispatcher
    }

    /// Runs `driver`'s polls on every status change from now on, and once
    /// right away, which sets its baseline.
    pub(crate) fn register(&self, driver: Weak<dyn StatusDriver>) {
        if let Ok(mut drivers) = self.drivers.lock() {
            drivers.push(driver);
        }
        self.notify();
    }

    /// Marks a pass pending and wakes the thread. Safe to call from a
    /// status hook: it takes no lock but its own.
    pub(crate) fn notify(&self) {
        if let Ok(mut pending) = self.pending.lock() {
            pending.status = true;
            if pending.waiting {
                self.wake.notify_one();
            }
        }
    }

    /// Ends the thread after the pass it is in.
    /// A listener was set or replaced on an entity of `runtime`: one pass,
    /// so a status that changed while no listener was there to take it is
    /// handed to the new one now. Without it the change waited for the
    /// next change of any status — a match discovery had already made
    /// before `set_listener` was never heard if nothing moved after it.
    pub(crate) fn listener_set(runtime: Option<&Arc<crate::runtime::DcpsRuntime>>) {
        LISTENER_GENERATION.fetch_add(1, Ordering::AcqRel);
        if let Some(runtime) = runtime {
            runtime.listener_dispatcher().notify();
        }
    }

    pub(crate) fn stop(&self) {
        self.stop.store(true, Ordering::Release);
        self.notify();
    }

    fn run(self: Arc<Self>) {
        loop {
            {
                let Ok(mut pending) = self.pending.lock() else {
                    return;
                };
                while !pending.status && !self.stop.load(Ordering::Acquire) {
                    pending.waiting = true;
                    pending = match self.wake.wait(pending) {
                        Ok(guard) => guard,
                        Err(_) => return,
                    };
                    pending.waiting = false;
                }
                pending.status = false;
            }
            if self.stop.load(Ordering::Acquire) {
                return;
            }
            // Upgrade under the lock, drive outside it: a listener may
            // create an entity, which registers here.
            let live: Vec<Arc<dyn StatusDriver>> = match self.drivers.lock() {
                Ok(mut drivers) => {
                    drivers.retain(|driver| driver.strong_count() > 0);
                    drivers.iter().filter_map(Weak::upgrade).collect()
                }
                Err(_) => return,
            };
            for driver in live {
                driver.drive();
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::time::{Duration, Instant};

    struct Counter(AtomicUsize);
    impl StatusDriver for Counter {
        fn drive(&self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn until(what: impl Fn() -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if what() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        false
    }

    #[test]
    fn a_registered_driver_runs_once_on_registration_and_once_per_notify() {
        let dispatcher = ListenerDispatcher::start();
        let counter = Arc::new(Counter(AtomicUsize::new(0)));
        let weak: Weak<dyn StatusDriver> = Arc::downgrade(&counter) as Weak<dyn StatusDriver>;
        dispatcher.register(weak);
        assert!(until(|| counter.0.load(Ordering::SeqCst) >= 1));
        let before = counter.0.load(Ordering::SeqCst);
        dispatcher.notify();
        assert!(until(|| counter.0.load(Ordering::SeqCst) > before));
        dispatcher.stop();
    }

    #[test]
    fn a_dropped_driver_is_not_kept_alive_or_run() {
        let dispatcher = ListenerDispatcher::start();
        let counter = Arc::new(Counter(AtomicUsize::new(0)));
        dispatcher.register(Arc::downgrade(&counter) as Weak<dyn StatusDriver>);
        assert!(until(|| counter.0.load(Ordering::SeqCst) >= 1));
        let weak = Arc::downgrade(&counter);
        drop(counter);
        assert!(weak.upgrade().is_none(), "the dispatcher kept it alive");
        dispatcher.notify();
        assert!(until(|| dispatcher.drivers.lock().unwrap().is_empty()));
        dispatcher.stop();
    }
}
