// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Runnable threadpool with priority **lanes** (RT-CORBA §5.7) — the
//! runtime realization of the structural model from [`crate::policy`].
//!
//! A [`ThreadpoolRuntime`] maps each [`Lane`](crate::policy::Lane) onto real
//! OS threads: per lane, `static_threads` workers are created at startup and up
//! to `dynamic_threads` more are added dynamically under saturation (with
//! idle-timeout teardown). [`dispatch`](ThreadpoolRuntime::dispatch) routes a
//! job via [`Threadpool::lane_for`](crate::policy::Threadpool::lane_for) into the
//! lane of its priority. All workers wait **event-driven** on a condvar
//! (no busy-poll).
//!
//! The **OS scheduler priority** (e.g. `pthread_setschedparam`/`SCHED_FIFO`)
//! is set via the injectable [`NativePrioritySetter`] hook — the
//! platform-specific, possibly `unsafe` code needed there deliberately lives in
//! the caller, so that this crate stays `forbid(unsafe_code)`.

#[cfg(feature = "std")]
pub use std_impl::{DispatchError, NativePrioritySetter, ThreadpoolRuntime};

#[cfg(feature = "std")]
#[allow(clippy::expect_used)]
mod std_impl {
    use alloc::boxed::Box;
    use alloc::collections::VecDeque;
    use alloc::sync::Arc;
    use alloc::vec::Vec;
    use core::fmt;
    use core::time::Duration;
    use std::sync::{Condvar, Mutex};
    use std::thread::JoinHandle;

    use crate::policy::Threadpool;
    use crate::priority::{Priority, PriorityMapping};

    /// Idle time after which a *dynamic* worker tears itself down.
    const DYNAMIC_IDLE_TIMEOUT: Duration = Duration::from_millis(100);

    type Job = Box<dyn FnOnce() + Send + 'static>;

    /// Hook for setting the native OS scheduler priority of a worker thread.
    ///
    /// Called once per worker at thread start, with the lane's native priority
    /// computed via the [`PriorityMapping`]. The concrete
    /// (platform-specific, often `unsafe`) implementation — e.g.
    /// `pthread_setschedparam` with `SCHED_FIFO` — is provided by the caller.
    pub trait NativePrioritySetter: Send + Sync {
        /// Sets the OS priority of the current thread.
        fn set_current_thread_priority(&self, native_priority: i32);
    }

    /// Error from [`dispatch`](ThreadpoolRuntime::dispatch).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum DispatchError {
        /// The pool has no lane (empty threadpool).
        NoLane,
        /// No worker free and buffering off/buffer full — request rejected.
        Rejected,
    }

    impl fmt::Display for DispatchError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::NoLane => f.write_str("threadpool has no lane"),
                Self::Rejected => f.write_str("request rejected (no worker, buffering off/full)"),
            }
        }
    }

    impl std::error::Error for DispatchError {}

    struct LaneState {
        queue: VecDeque<Job>,
        /// Currently alive workers (static + dynamic).
        workers: u32,
        /// Workers currently executing a job.
        busy: u32,
        /// Of those, the dynamically created ones (for the growth budget).
        dynamic_alive: u32,
        shutdown: bool,
    }

    struct Lane {
        priority: Priority,
        native_priority: i32,
        dynamic_threads: u32,
        sync: Arc<(Mutex<LaneState>, Condvar)>,
        handles: Mutex<Vec<JoinHandle<()>>>,
    }

    /// A running threadpool with priority lanes (RT-CORBA §5.7).
    pub struct ThreadpoolRuntime {
        lanes: Vec<Lane>,
        allow_buffering: bool,
        max_buffered: u32,
        hook: Option<Arc<dyn NativePrioritySetter>>,
    }

    impl ThreadpoolRuntime {
        /// Starts a threadpool: creates `static_threads` workers per lane.
        ///
        /// `mapping` maps the lane priority to the native priority that is
        /// passed to the `hook` (if set) per worker.
        #[must_use]
        pub fn start<M: PriorityMapping>(
            pool: &Threadpool,
            mapping: &M,
            hook: Option<Arc<dyn NativePrioritySetter>>,
        ) -> Self {
            let mut lanes = Vec::with_capacity(pool.lanes.len());
            for lane_cfg in &pool.lanes {
                let native_priority = mapping.to_native(lane_cfg.priority).unwrap_or(0);
                let lane = Lane {
                    priority: lane_cfg.priority,
                    native_priority,
                    dynamic_threads: lane_cfg.dynamic_threads,
                    sync: Arc::new((
                        Mutex::new(LaneState {
                            queue: VecDeque::new(),
                            workers: lane_cfg.static_threads,
                            busy: 0,
                            dynamic_alive: 0,
                            shutdown: false,
                        }),
                        Condvar::new(),
                    )),
                    handles: Mutex::new(Vec::new()),
                };
                let stacksize = pool.stacksize;
                let mut handles = lane.handles.lock().expect("lane handles poisoned");
                for _ in 0..lane_cfg.static_threads {
                    handles.push(spawn_worker(
                        Arc::clone(&lane.sync),
                        hook.clone(),
                        native_priority,
                        stacksize,
                        false,
                    ));
                }
                drop(handles);
                lanes.push(lane);
            }
            Self {
                lanes,
                allow_buffering: pool.allow_request_buffering,
                max_buffered: pool.max_buffered_requests,
                hook,
            }
        }

        /// Selects the lane index for a priority — same rule as
        /// [`Threadpool::lane_for`](crate::policy::Threadpool::lane_for):
        /// highest lane priority ≤ `priority`, otherwise the lowest lane.
        fn select_lane(&self, priority: Priority) -> Option<usize> {
            let covering = self
                .lanes
                .iter()
                .enumerate()
                .filter(|(_, l)| l.priority <= priority)
                .max_by_key(|(_, l)| l.priority)
                .map(|(i, _)| i);
            covering.or_else(|| {
                self.lanes
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, l)| l.priority)
                    .map(|(i, _)| i)
            })
        }

        /// Hands a job to the lane of its priority. Wakes a waiting worker or
        /// creates — under saturation with free dynamic budget — a dynamic
        /// worker.
        ///
        /// # Errors
        /// [`DispatchError::NoLane`] on an empty pool; [`DispatchError::Rejected`]
        /// if no worker is free and buffering is off or the buffer is full.
        #[allow(clippy::missing_panics_doc)]
        pub fn dispatch<F>(&self, priority: Priority, job: F) -> Result<(), DispatchError>
        where
            F: FnOnce() + Send + 'static,
        {
            let idx = self.select_lane(priority).ok_or(DispatchError::NoLane)?;
            let lane = &self.lanes[idx];
            let (lock, cv) = &*lane.sync;

            let need_spawn;
            {
                let mut st = lock.lock().expect("lane state poisoned");
                // Free capacity = alive workers minus already-scheduled work
                // (running + in the queue). A worker that is just starting up
                // counts as free; a waiting job occupies a worker.
                let pending = st.busy + st.queue.len() as u32;
                let free = st.workers.saturating_sub(pending);
                let can_grow = st.dynamic_alive < lane.dynamic_threads;
                if !self.allow_buffering && free == 0 && !can_grow {
                    return Err(DispatchError::Rejected);
                }
                if self.max_buffered > 0 && st.queue.len() as u32 >= self.max_buffered {
                    return Err(DispatchError::Rejected);
                }
                st.queue.push_back(Box::new(job));
                need_spawn = free == 0 && can_grow;
                if need_spawn {
                    st.workers += 1;
                    st.dynamic_alive += 1;
                }
            }

            if need_spawn {
                let handle = spawn_worker(
                    Arc::clone(&lane.sync),
                    self.hook.clone(),
                    lane.native_priority,
                    0,
                    true,
                );
                lane.handles
                    .lock()
                    .expect("lane handles poisoned")
                    .push(handle);
            } else {
                cv.notify_one();
            }
            Ok(())
        }

        /// Number of lanes.
        #[must_use]
        pub fn lane_count(&self) -> usize {
            self.lanes.len()
        }

        /// Number of currently alive workers of a lane (static + dynamic),
        /// derived from the stored handles. Primarily for tests/telemetry.
        ///
        /// # Panics
        /// On a poisoned internal lock.
        #[must_use]
        pub fn spawned_workers(&self, lane_index: usize) -> usize {
            self.lanes
                .get(lane_index)
                .map(|l| l.handles.lock().expect("lane handles poisoned").len())
                .unwrap_or(0)
        }
    }

    impl Drop for ThreadpoolRuntime {
        fn drop(&mut self) {
            for lane in &self.lanes {
                let (lock, cv) = &*lane.sync;
                {
                    let mut st = lock.lock().expect("lane state poisoned");
                    st.shutdown = true;
                }
                cv.notify_all();
            }
            for lane in &self.lanes {
                let handles =
                    core::mem::take(&mut *lane.handles.lock().expect("lane handles poisoned"));
                for h in handles {
                    let _ = h.join();
                }
            }
        }
    }

    fn spawn_worker(
        sync: Arc<(Mutex<LaneState>, Condvar)>,
        hook: Option<Arc<dyn NativePrioritySetter>>,
        native_priority: i32,
        stacksize: usize,
        dynamic: bool,
    ) -> JoinHandle<()> {
        let mut builder =
            std::thread::Builder::new().name(alloc::format!("rtcorba-lane-{native_priority}"));
        if stacksize > 0 {
            builder = builder.stack_size(stacksize);
        }
        builder
            .spawn(move || worker_loop(&sync, hook.as_deref(), native_priority, dynamic))
            .expect("spawn rt-corba worker")
    }

    fn worker_loop(
        sync: &(Mutex<LaneState>, Condvar),
        hook: Option<&dyn NativePrioritySetter>,
        native_priority: i32,
        dynamic: bool,
    ) {
        if let Some(h) = hook {
            h.set_current_thread_priority(native_priority);
        }
        let (lock, cv) = sync;
        loop {
            let job = {
                let mut st = lock.lock().expect("lane state poisoned");
                loop {
                    if let Some(job) = st.queue.pop_front() {
                        st.busy += 1;
                        break job;
                    }
                    if st.shutdown {
                        st.workers = st.workers.saturating_sub(1);
                        if dynamic {
                            st.dynamic_alive = st.dynamic_alive.saturating_sub(1);
                        }
                        return;
                    }
                    if dynamic {
                        let (guard, timeout) = cv
                            .wait_timeout(st, DYNAMIC_IDLE_TIMEOUT)
                            .expect("lane state poisoned");
                        st = guard;
                        if timeout.timed_out() && st.queue.is_empty() && !st.shutdown {
                            st.workers = st.workers.saturating_sub(1);
                            st.dynamic_alive = st.dynamic_alive.saturating_sub(1);
                            return;
                        }
                    } else {
                        st = cv.wait(st).expect("lane state poisoned");
                    }
                }
            };
            job();
            let mut st = lock.lock().expect("lane state poisoned");
            st.busy = st.busy.saturating_sub(1);
        }
    }
}

#[cfg(all(test, feature = "std"))]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::policy::{Lane, Threadpool};
    use crate::priority::{LinearPriorityMapping, Priority};
    use alloc::sync::Arc;
    use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};

    fn p(v: i16) -> Priority {
        Priority::new(v).unwrap()
    }

    fn pool() -> Threadpool {
        Threadpool {
            lanes: alloc::vec![
                Lane {
                    priority: p(0),
                    static_threads: 1,
                    dynamic_threads: 0,
                },
                Lane {
                    priority: p(50),
                    static_threads: 2,
                    dynamic_threads: 2,
                },
            ],
            stacksize: 0,
            allow_request_buffering: true,
            max_buffered_requests: 0,
        }
    }

    #[test]
    fn dispatches_and_runs_all_jobs() {
        let counter = Arc::new(AtomicU32::new(0));
        let rt = ThreadpoolRuntime::start(&pool(), &LinearPriorityMapping::new(1, 99), None);
        for _ in 0..20 {
            let c = Arc::clone(&counter);
            rt.dispatch(p(60), move || {
                c.fetch_add(1, Ordering::SeqCst);
            })
            .unwrap();
        }
        drop(rt); // joins all workers → all jobs have been processed
        assert_eq!(counter.load(Ordering::SeqCst), 20);
    }

    #[test]
    fn routes_to_lane_by_priority() {
        let seen = Arc::new(AtomicI32::new(-1));
        let hook_pool = pool();
        let rt = ThreadpoolRuntime::start(&hook_pool, &LinearPriorityMapping::new(1, 99), None);
        // Priority 10 → covers only lane 0 (prio 0).
        let s = Arc::clone(&seen);
        let done = Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
        let d2 = Arc::clone(&done);
        rt.dispatch(p(10), move || {
            // Lane-0 workers run at the native prio of Priority(0) = 1.
            s.store(1, Ordering::SeqCst);
            let (m, cv) = &*d2;
            *m.lock().unwrap() = true;
            cv.notify_all();
        })
        .unwrap();
        let (m, cv) = &*done;
        let mut g = m.lock().unwrap();
        while !*g {
            g = cv.wait(g).unwrap();
        }
        assert_eq!(seen.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn native_priority_hook_invoked_per_worker() {
        struct RecordHook(Arc<std::sync::Mutex<alloc::vec::Vec<i32>>>);
        impl NativePrioritySetter for RecordHook {
            fn set_current_thread_priority(&self, native_priority: i32) {
                self.0.lock().unwrap().push(native_priority);
            }
        }
        let log = Arc::new(std::sync::Mutex::new(alloc::vec::Vec::new()));
        let hook = Arc::new(RecordHook(Arc::clone(&log)));
        let rt = ThreadpoolRuntime::start(
            &pool(),
            &LinearPriorityMapping::new(1, 99),
            Some(hook as Arc<dyn NativePrioritySetter>),
        );
        drop(rt);
        let mut got = log.lock().unwrap().clone();
        got.sort_unstable();
        // Lane 0 (prio 0 → native 1) ×1 worker, lane 50 (→ ~50) ×2 workers.
        assert_eq!(got.len(), 3);
        assert_eq!(got[0], 1);
    }

    #[test]
    fn rejects_when_buffering_off_and_no_worker() {
        let mut tp = Threadpool {
            lanes: alloc::vec![Lane {
                priority: p(0),
                static_threads: 1,
                dynamic_threads: 0,
            }],
            stacksize: 0,
            allow_request_buffering: false,
            max_buffered_requests: 0,
        };
        tp.allow_request_buffering = false;
        let rt = ThreadpoolRuntime::start(&tp, &LinearPriorityMapping::new(1, 99), None);
        // Occupy the single worker with a blocking job.
        let gate = Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
        let g2 = Arc::clone(&gate);
        rt.dispatch(p(0), move || {
            let (m, cv) = &*g2;
            let mut held = m.lock().unwrap();
            while !*held {
                held = cv.wait(held).unwrap();
            }
        })
        .unwrap();
        // Wait until the worker has picked up the job (idle == 0).
        std::thread::sleep(std::time::Duration::from_millis(50));
        // Now no worker is free, no dynamic budget, buffering off → reject.
        let r = rt.dispatch(p(0), || {});
        assert_eq!(r, Err(DispatchError::Rejected));
        // Open the gate so the worker ends cleanly.
        let (m, cv) = &*gate;
        *m.lock().unwrap() = true;
        cv.notify_all();
    }

    #[test]
    fn dynamic_worker_spawns_under_saturation() {
        // Lane 50: 2 static + 2 dynamic. 4 blocking jobs must all be able to
        // run at the same time → 2 dynamic workers are created.
        let rt = ThreadpoolRuntime::start(&pool(), &LinearPriorityMapping::new(1, 99), None);
        let running = Arc::new(AtomicU32::new(0));
        let gate = Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
        for _ in 0..4 {
            let r = Arc::clone(&running);
            let g = Arc::clone(&gate);
            rt.dispatch(p(50), move || {
                r.fetch_add(1, Ordering::SeqCst);
                let (m, cv) = &*g;
                let mut held = m.lock().unwrap();
                while !*held {
                    held = cv.wait(held).unwrap();
                }
            })
            .unwrap();
        }
        // Until all 4 run at the same time (otherwise there would be only 2 workers).
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while running.load(Ordering::SeqCst) < 4 {
            assert!(
                std::time::Instant::now() < deadline,
                "dynamic workers spawned too few"
            );
            std::thread::yield_now();
        }
        assert_eq!(running.load(Ordering::SeqCst), 4);
        // Lane 50 is index 1: 2 static + 2 dynamic = 4 handles.
        assert_eq!(rt.spawned_workers(1), 4);
        let (m, cv) = &*gate;
        *m.lock().unwrap() = true;
        cv.notify_all();
    }
}
