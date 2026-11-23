// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The threading policy (DDS4CCM 1.1 §7.4.4): which threads call the
//! facets of a DDS port.
//!
//! "The threading policy is local to the component using a DDS port" and
//! "could be set at several levels": the port "(for all its facets)", the
//! component "(for all the facets of its ports)", the container "(for all
//! the facets of its components' ports)". When a facet is activated, the
//! thread pool of the port is used; "if there is no port's policy, the
//! component's threadpool is used", "if there is no component's one, the
//! container's threadpool is used", "if there is no container's policy,
//! then the default is applied".
//!
//! The specification does not say what a policy holds. Here it is the
//! number of threads of the pool ([`ThreadingPolicy`]). The facets of a
//! port are the interfaces the component provides to it: `data_listener`
//! (`Listener`, `StateListener`) and `status` (`PortStatusListener`).
//!
//! - A port with a policy has a pool of its own.
//! - A component with a policy has one pool, shared by its ports without
//!   a policy.
//! - A container with a policy has one pool, shared by the ports of its
//!   components that have none either.
//! - The default, without any policy: no pool. The facet is called on the
//!   thread that brings the event: for data and statuses the thread DDS
//!   reports them on (a thread of DDS, or the thread of a DataWriter of
//!   the same process), for the data a port already holds when the
//!   listener is connected or enabled the thread of the caller.
//!
//! A pool with several threads does not call the facets of one port
//! concurrently and does not reorder them: the calls of a port are queued
//! per port, and one thread at a time works a port's queue. Several
//! threads serve several ports at once.

use std::collections::VecDeque;
use std::num::NonZeroUsize;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};

use crate::reader_core::lock;

/// A threading policy: the size of the thread pool that calls the facets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThreadingPolicy {
    /// The number of threads of the pool.
    pub threads: NonZeroUsize,
}

impl ThreadingPolicy {
    /// A pool of one thread.
    pub const SINGLE: Self = Self {
        threads: NonZeroUsize::MIN,
    };

    /// A pool of `threads` threads; `None` for zero.
    #[must_use]
    pub fn threads(threads: usize) -> Option<Self> {
        NonZeroUsize::new(threads).map(|threads| Self { threads })
    }
}

type Job = Box<dyn FnOnce() + Send + 'static>;

struct Queue {
    /// The jobs, and whether the pool is closed.
    jobs: Mutex<(VecDeque<Job>, bool)>,
    ready: Condvar,
}

/// The thread pool of a port, a component or a container.
///
/// Its threads are named `<name>-<index>`. They end when the last holder
/// of the pool lets go of it, after the calls that are queued.
pub struct ThreadPool {
    name: String,
    policy: ThreadingPolicy,
    queue: Arc<Queue>,
    workers: Mutex<Vec<JoinHandle<()>>>,
}

impl std::fmt::Debug for ThreadPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThreadPool")
            .field("name", &self.name)
            .field("policy", &self.policy)
            .finish_non_exhaustive()
    }
}

impl ThreadPool {
    /// Starts the pool `name` with the threads of `policy`.
    ///
    /// # Errors
    /// The error of the operating system if a thread does not start; the
    /// threads started before it are ended.
    pub fn new(name: &str, policy: ThreadingPolicy) -> std::io::Result<Arc<Self>> {
        let pool = Self {
            name: name.to_owned(),
            policy,
            queue: Arc::new(Queue {
                jobs: Mutex::new((VecDeque::new(), false)),
                ready: Condvar::new(),
            }),
            workers: Mutex::new(Vec::new()),
        };
        for index in 0..policy.threads.get() {
            let queue = Arc::clone(&pool.queue);
            // On an error `pool` is dropped, which ends the threads so far.
            let worker = thread::Builder::new()
                .name(format!("{name}-{index}"))
                .spawn(move || work(&queue))?;
            lock(&pool.workers).push(worker);
        }
        Ok(Arc::new(pool))
    }

    /// The name of the pool, the prefix of the names of its threads.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The policy the pool was started with.
    #[must_use]
    pub fn policy(&self) -> ThreadingPolicy {
        self.policy
    }

    fn execute(&self, job: Job) {
        lock(&self.queue.jobs).0.push_back(job);
        self.queue.ready.notify_one();
    }
}

fn work(queue: &Queue) {
    loop {
        let job = {
            let mut jobs = lock(&queue.jobs);
            loop {
                if let Some(job) = jobs.0.pop_front() {
                    break job;
                }
                if jobs.1 {
                    return;
                }
                jobs = queue
                    .ready
                    .wait(jobs)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
        };
        // A job that panics does not take the thread with it: the pool
        // keeps the size of its policy.
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        lock(&self.queue.jobs).1 = true;
        self.queue.ready.notify_all();
        let current = thread::current().id();
        for worker in lock(&self.workers).drain(..) {
            // The last holder may be a call on one of the threads.
            if worker.thread().id() != current {
                let _ = worker.join();
            }
        }
    }
}

struct Strand {
    pool: Arc<ThreadPool>,
    /// The calls of the port that wait, and whether a thread of the pool
    /// is working them or asked to.
    waiting: Mutex<(VecDeque<Job>, bool)>,
}

impl Strand {
    fn post(self: &Arc<Self>, job: Job) {
        let mut waiting = lock(&self.waiting);
        waiting.0.push_back(job);
        if waiting.1 {
            return;
        }
        waiting.1 = true;
        drop(waiting);
        let strand = Arc::clone(self);
        self.pool.execute(Box::new(move || strand.drain()));
    }

    fn drain(&self) {
        loop {
            let job = {
                let mut waiting = lock(&self.waiting);
                match waiting.0.pop_front() {
                    Some(job) => job,
                    None => {
                        waiting.1 = false;
                        return;
                    }
                }
            };
            // A facet that panics takes its call with it, not the queue of
            // the port: the calls behind it are made.
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
        }
    }
}

/// Where the facets of one port are called: on the thread that brings the
/// event (the default), or one after the other on a thread pool.
#[derive(Clone, Default)]
pub(crate) struct Facets(Option<Arc<Strand>>);

impl Facets {
    /// The facets of a port on `pool`; `None` is the default.
    pub(crate) fn on(pool: Option<Arc<ThreadPool>>) -> Self {
        Self(pool.map(|pool| {
            Arc::new(Strand {
                pool,
                waiting: Mutex::new((VecDeque::new(), false)),
            })
        }))
    }

    /// The pool the facets are called on; `None` for the default.
    pub(crate) fn pool(&self) -> Option<&Arc<ThreadPool>> {
        self.0.as_ref().map(|strand| &strand.pool)
    }

    /// Calls a facet: `call` runs now on this thread, or after the calls
    /// of the port queued before it on a thread of the pool.
    pub(crate) fn call(&self, call: impl FnOnce() + Send + 'static) {
        match &self.0 {
            None => call(),
            Some(strand) => strand.post(Box::new(call)),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;
    use std::time::Duration;

    const WAIT: Duration = Duration::from_secs(10);

    #[test]
    fn dds4ccm_7_4_4_a_pool_has_the_threads_of_its_policy_under_its_name() {
        let policy = ThreadingPolicy::threads(3).unwrap();
        let pool = ThreadPool::new("probe", policy).unwrap();
        assert_eq!(pool.policy(), policy);
        assert_eq!(pool.name(), "probe");
        // Three calls that each hold their thread until all three run:
        // they need three threads.
        let (names, seen) = mpsc::channel();
        let barrier = Arc::new(std::sync::Barrier::new(3));
        for _ in 0..3 {
            let (names, barrier) = (names.clone(), Arc::clone(&barrier));
            pool.execute(Box::new(move || {
                barrier.wait();
                let name = thread::current().name().map(str::to_owned);
                names.send(name).unwrap();
            }));
        }
        let mut names: Vec<_> = (0..3)
            .map(|_| seen.recv_timeout(WAIT).unwrap().unwrap())
            .collect();
        names.sort();
        assert_eq!(names, ["probe-0", "probe-1", "probe-2"]);
        assert!(ThreadingPolicy::threads(0).is_none());
        assert_eq!(ThreadingPolicy::SINGLE.threads.get(), 1);
    }

    #[test]
    fn dds4ccm_7_4_4_without_a_pool_a_facet_is_called_on_the_thread_of_the_event() {
        let facets = Facets::on(None);
        assert!(facets.pool().is_none());
        let (thread_of_call, seen) = mpsc::channel();
        facets.call(move || thread_of_call.send(thread::current().id()).unwrap());
        // Called before `call` returned, on this thread.
        assert_eq!(seen.try_recv().unwrap(), thread::current().id());
    }

    #[test]
    fn dds4ccm_7_4_4_the_facets_of_one_port_are_called_in_order_and_one_at_a_time() {
        let pool = ThreadPool::new("ordered", ThreadingPolicy::threads(4).unwrap()).unwrap();
        let facets = Facets::on(Some(Arc::clone(&pool)));
        assert!(Arc::ptr_eq(facets.pool().unwrap(), &pool));
        let inside = Arc::new(AtomicUsize::new(0));
        let (order, seen) = mpsc::channel();
        for call in 0..200_usize {
            let (inside, order) = (Arc::clone(&inside), order.clone());
            facets.call(move || {
                let others = inside.fetch_add(1, Ordering::SeqCst);
                thread::yield_now();
                inside.fetch_sub(1, Ordering::SeqCst);
                order.send((call, others)).unwrap();
            });
        }
        for expected in 0..200_usize {
            let (call, others) = seen.recv_timeout(WAIT).unwrap();
            assert_eq!(call, expected, "the calls keep their order");
            assert_eq!(others, 0, "no two calls of the port at once");
        }
    }

    #[test]
    fn dds4ccm_7_4_4_two_ports_of_one_pool_are_served_at_once() {
        let pool = ThreadPool::new("shared", ThreadingPolicy::threads(2).unwrap()).unwrap();
        let first = Facets::on(Some(Arc::clone(&pool)));
        let second = Facets::on(Some(Arc::clone(&pool)));
        // Each call waits for the other: with one port at a time this
        // would not end.
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let (done, seen) = mpsc::channel();
        for facets in [&first, &second] {
            let (barrier, done) = (Arc::clone(&barrier), done.clone());
            facets.call(move || {
                barrier.wait();
                done.send(()).unwrap();
            });
        }
        seen.recv_timeout(WAIT).unwrap();
        seen.recv_timeout(WAIT).unwrap();
    }

    #[test]
    fn dds4ccm_7_4_4_a_panicking_facet_does_not_cost_the_pool_its_thread() {
        let pool = ThreadPool::new("sturdy", ThreadingPolicy::SINGLE).unwrap();
        let facets = Facets::on(Some(pool));
        facets.call(|| std::panic::resume_unwind(Box::new("the facet failed")));
        let (after, seen) = mpsc::channel();
        facets.call(move || {
            after
                .send(thread::current().name().map(str::to_owned))
                .unwrap()
        });
        assert_eq!(
            seen.recv_timeout(WAIT).unwrap().as_deref(),
            Some("sturdy-0")
        );
    }

    #[test]
    fn dds4ccm_7_4_4_a_pool_let_go_of_runs_what_is_queued_and_ends_its_threads() {
        let pool = ThreadPool::new("ending", ThreadingPolicy::SINGLE).unwrap();
        let ran = Arc::new(AtomicUsize::new(0));
        for _ in 0..50 {
            let ran = Arc::clone(&ran);
            pool.execute(Box::new(move || {
                ran.fetch_add(1, Ordering::SeqCst);
            }));
        }
        // Dropping joins the threads, which first empty the queue.
        drop(pool);
        assert_eq!(ran.load(Ordering::SeqCst), 50);
    }
}
