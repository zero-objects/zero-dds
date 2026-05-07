// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! TimerEventService — Spec OMG Time 1.1 §2.2-§2.4.
//!
//! Liefert One-Shot- und Periodic-Timer, die in einen
//! Container-Worker-Thread eingebettet sind. Bei Feuerung wird ein
//! `TimerCallback` aufgerufen.

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Timer-Kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerKind {
    /// One-Shot: feuert genau einmal.
    OneShot,
    /// Periodic: feuert wiederholt mit dem Periode-Intervall.
    Periodic,
}

/// Timer-Handle (vom Service vergeben).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimerHandle(pub u64);

/// Callback-Trait fuer Timer-Feuerung.
pub trait TimerCallback: Send + Sync {
    /// Wird vom Service-Thread aufgerufen, wenn der Timer feuert.
    fn fire(&self, handle: TimerHandle);
}

struct TimerEntry {
    kind: TimerKind,
    next_fire: Instant,
    period: Duration,
    callback: Arc<dyn TimerCallback>,
}

struct ServiceInner {
    next_handle: AtomicU64,
    timers: Mutex<BTreeMap<TimerHandle, TimerEntry>>,
    shutdown: AtomicBool,
}

/// TimerEventService — startet einen Worker-Thread.
pub struct TimerEventService {
    inner: Arc<ServiceInner>,
    worker: Option<JoinHandle<()>>,
}

impl core::fmt::Debug for TimerEventService {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let n = self.inner.timers.lock().ok().map(|g| g.len()).unwrap_or(0);
        f.debug_struct("TimerEventService")
            .field("active_timers", &n)
            .finish()
    }
}

impl Default for TimerEventService {
    fn default() -> Self {
        Self::new()
    }
}

impl TimerEventService {
    /// Konstruktor — startet Worker-Thread.
    #[must_use]
    pub fn new() -> Self {
        let inner = Arc::new(ServiceInner {
            next_handle: AtomicU64::new(1),
            timers: Mutex::new(BTreeMap::new()),
            shutdown: AtomicBool::new(false),
        });
        let inner_w = Arc::clone(&inner);
        let worker = thread::Builder::new()
            .name("ccm-timer-service".into())
            .spawn(move || run_worker(&inner_w))
            .ok();
        Self { inner, worker }
    }

    /// Erstellt einen One-Shot-Timer, der nach `delay` feuert.
    pub fn create_one_shot(&self, delay: Duration, cb: Arc<dyn TimerCallback>) -> TimerHandle {
        self.create_internal(TimerKind::OneShot, delay, delay, cb)
    }

    /// Erstellt einen Periodic-Timer mit `period`-Intervall.
    pub fn create_periodic(&self, period: Duration, cb: Arc<dyn TimerCallback>) -> TimerHandle {
        self.create_internal(TimerKind::Periodic, period, period, cb)
    }

    fn create_internal(
        &self,
        kind: TimerKind,
        delay: Duration,
        period: Duration,
        callback: Arc<dyn TimerCallback>,
    ) -> TimerHandle {
        let handle = TimerHandle(self.inner.next_handle.fetch_add(1, Ordering::Relaxed));
        let entry = TimerEntry {
            kind,
            next_fire: Instant::now() + delay,
            period,
            callback,
        };
        if let Ok(mut g) = self.inner.timers.lock() {
            g.insert(handle, entry);
        }
        handle
    }

    /// Cancelt einen Timer.
    pub fn cancel(&self, handle: TimerHandle) -> bool {
        self.inner
            .timers
            .lock()
            .map(|mut g| g.remove(&handle).is_some())
            .unwrap_or(false)
    }

    /// Anzahl aktiver Timer.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.inner.timers.lock().map(|g| g.len()).unwrap_or(0)
    }

    /// Stoppt den Service.
    pub fn shutdown(mut self) {
        self.inner.shutdown.store(true, Ordering::Release);
        if let Some(j) = self.worker.take() {
            let _ = j.join();
        }
    }
}

impl Drop for TimerEventService {
    fn drop(&mut self) {
        self.inner.shutdown.store(true, Ordering::Release);
        if let Some(j) = self.worker.take() {
            let _ = j.join();
        }
    }
}

fn run_worker(inner: &Arc<ServiceInner>) {
    let tick = Duration::from_millis(20);
    while !inner.shutdown.load(Ordering::Acquire) {
        let now = Instant::now();
        let mut to_fire: alloc::vec::Vec<(TimerHandle, Arc<dyn TimerCallback>)> = alloc::vec![];
        let mut to_remove: alloc::vec::Vec<TimerHandle> = alloc::vec![];
        if let Ok(mut g) = inner.timers.lock() {
            for (h, e) in g.iter_mut() {
                if e.next_fire <= now {
                    to_fire.push((*h, Arc::clone(&e.callback)));
                    match e.kind {
                        TimerKind::OneShot => to_remove.push(*h),
                        TimerKind::Periodic => e.next_fire = now + e.period,
                    }
                }
            }
            for h in &to_remove {
                g.remove(h);
            }
        }
        for (h, cb) in to_fire {
            cb.fire(h);
        }
        thread::sleep(tick);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use core::sync::atomic::AtomicUsize;

    struct CountingCallback {
        fired: Arc<AtomicUsize>,
    }
    impl TimerCallback for CountingCallback {
        fn fire(&self, _: TimerHandle) {
            self.fired.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn waitfor(c: &Arc<AtomicUsize>, target: usize, timeout: Duration) {
        let start = Instant::now();
        while c.load(Ordering::Relaxed) < target && start.elapsed() < timeout {
            thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn one_shot_fires_once() {
        let svc = TimerEventService::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let cb = Arc::new(CountingCallback {
            fired: Arc::clone(&counter),
        });
        let _ = svc.create_one_shot(Duration::from_millis(50), cb);
        waitfor(&counter, 1, Duration::from_secs(2));
        assert_eq!(counter.load(Ordering::Relaxed), 1);
        // Nach Feuerung ist der One-Shot-Timer raus.
        thread::sleep(Duration::from_millis(150));
        assert_eq!(svc.active_count(), 0);
    }

    #[test]
    fn periodic_fires_multiple_times() {
        let svc = TimerEventService::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let cb = Arc::new(CountingCallback {
            fired: Arc::clone(&counter),
        });
        let h = svc.create_periodic(Duration::from_millis(50), cb);
        waitfor(&counter, 3, Duration::from_secs(3));
        assert!(counter.load(Ordering::Relaxed) >= 3);
        svc.cancel(h);
    }

    #[test]
    fn cancel_stops_periodic() {
        let svc = TimerEventService::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let cb = Arc::new(CountingCallback {
            fired: Arc::clone(&counter),
        });
        let h = svc.create_periodic(Duration::from_millis(50), cb);
        thread::sleep(Duration::from_millis(150));
        assert!(svc.cancel(h));
        let after = counter.load(Ordering::Relaxed);
        thread::sleep(Duration::from_millis(200));
        assert_eq!(counter.load(Ordering::Relaxed), after);
    }

    #[test]
    fn cancel_unknown_returns_false() {
        let svc = TimerEventService::new();
        assert!(!svc.cancel(TimerHandle(9999)));
    }
}
