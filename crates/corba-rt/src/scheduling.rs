// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Static scheduling service (RT-CORBA scheduling profile): task model +
//! a-priori schedulability analysis for fixed-priority preemptive scheduling.
//!
//! Two tests over a periodic task set `(T_i, C_i, D_i)`:
//! - **Liu & Layland utilization bound** ([`TaskSet::is_schedulable_ll`]) —
//!   *sufficient* for rate-monotonic with implicit deadlines (`D = T`):
//!   `U = Σ C_i/T_i ≤ n·(2^(1/n) − 1)`.
//! - **Response-time analysis** ([`TaskSet::is_schedulable_rta`]) — *exact*
//!   (necessary + sufficient) for fixed-priority preemptive:
//!   `R_i = C_i + Σ_{j∈hp(i)} ⌈R_i/T_j⌉·C_j` as a fixed point, schedulable
//!   iff `R_i ≤ D_i` for all `i`.
//!
//! A certification-relevant profile (avionics/defense): it proves
//! *in advance* that all deadlines are met under worst-case load.

use alloc::vec::Vec;

use crate::priority::Priority;

/// A periodic (or sporadic) task in the static schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Task {
    /// Period `T` (time units, e.g. µs): minimum gap between two releases.
    pub period: u64,
    /// Worst-case execution time `C` (same unit as `period`).
    pub wcet: u64,
    /// Relative deadline `D` (`≤ T`). `0` → implicit deadline (`D = T`).
    pub deadline: u64,
    /// Fixed execution priority (higher = more urgent). Derivable from the
    /// periods via [`TaskSet::assign_rate_monotonic`].
    pub priority: Priority,
}

impl Task {
    /// Task with implicit deadline (`D = T`).
    #[must_use]
    pub fn new(period: u64, wcet: u64, priority: Priority) -> Self {
        Self {
            period,
            wcet,
            deadline: 0,
            priority,
        }
    }

    /// Task with explicit (constrained) deadline `D ≤ T`.
    #[must_use]
    pub fn with_deadline(period: u64, wcet: u64, deadline: u64, priority: Priority) -> Self {
        Self {
            period,
            wcet,
            deadline,
            priority,
        }
    }

    /// Effective relative deadline: `deadline`, or `period` if `deadline == 0`.
    #[must_use]
    pub fn effective_deadline(&self) -> u64 {
        if self.deadline == 0 {
            self.period
        } else {
            self.deadline
        }
    }
}

/// A task set for the static schedulability analysis.
#[derive(Debug, Clone, Default)]
pub struct TaskSet {
    /// The contained tasks.
    pub tasks: Vec<Task>,
}

/// `n`-th root of 2 via Newton iteration — `no_std`-capable, uses only
/// basic f64 operations (no `powf`/`ln` from `std`).
#[must_use]
fn nth_root_of_two(n: u32) -> f64 {
    if n == 0 {
        return 1.0;
    }
    if n == 1 {
        return 2.0;
    }
    let nf = f64::from(n);
    let mut x = 1.0_f64;
    // f(x) = x^n − 2, x_{k+1} = x_k − (x_k^n − 2)/(n·x_k^{n−1}).
    let mut iter = 0;
    while iter < 100 {
        let mut x_nm1 = 1.0_f64; // x^{n-1}
        let mut k = 0;
        while k < n - 1 {
            x_nm1 *= x;
            k += 1;
        }
        let x_n = x_nm1 * x;
        let denom = nf * x_nm1;
        if denom == 0.0 {
            break;
        }
        let next = x - (x_n - 2.0) / denom;
        let delta = if next > x { next - x } else { x - next };
        x = next;
        if delta < 1e-13 {
            break;
        }
        iter += 1;
    }
    x
}

impl TaskSet {
    /// Empty task set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a task.
    pub fn push(&mut self, task: Task) {
        self.tasks.push(task);
    }

    /// Number of tasks.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    /// Whether the task set is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    /// Total utilization `U = Σ C_i/T_i`.
    #[must_use]
    pub fn utilization(&self) -> f64 {
        self.tasks
            .iter()
            .filter(|t| t.period > 0)
            .map(|t| t.wcet as f64 / t.period as f64)
            .sum()
    }

    /// The Liu & Layland utilization bound `n·(2^(1/n) − 1)` for `n` tasks.
    #[must_use]
    pub fn liu_layland_bound(n: usize) -> f64 {
        if n == 0 {
            return 1.0;
        }
        let nf = n as f64;
        nf * (nth_root_of_two(n as u32) - 1.0)
    }

    /// Assigns rate-monotonic priorities: shorter period → higher priority.
    /// The priorities are assigned evenly across `0..=MAX` in order
    /// (unique, as long as ≤ 32768 tasks).
    pub fn assign_rate_monotonic(&mut self) {
        let n = self.tasks.len();
        if n == 0 {
            return;
        }
        // Indices by ascending period (shortest first = most urgent).
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by_key(|&i| self.tasks[i].period);
        // Highest rank → highest priority. Unique, monotonically decreasing values.
        for (rank, &idx) in order.iter().enumerate() {
            let prio_val = (n - 1 - rank) as i32;
            self.tasks[idx].priority = Priority::clamped(prio_val);
        }
    }

    /// Higher-priority set `hp(i)`: tasks with *strictly higher* priority than
    /// task `i` (they preempt `i`).
    fn higher_priority_than(&self, i: usize) -> impl Iterator<Item = &Task> {
        let pi = self.tasks[i].priority;
        self.tasks
            .iter()
            .enumerate()
            .filter(move |(j, t)| *j != i && t.priority > pi)
            .map(|(_, t)| t)
    }

    /// Exact worst-case response time `R_i` of task `i` via fixed-point iteration.
    /// `None` if `R_i` exceeds the deadline (task not schedulable).
    #[must_use]
    pub fn response_time(&self, i: usize) -> Option<u64> {
        if i >= self.tasks.len() {
            return None;
        }
        let ti = self.tasks[i];
        let deadline = ti.effective_deadline();
        let mut r = ti.wcet;
        loop {
            let interference: u64 = self
                .higher_priority_than(i)
                .map(|t| r.div_ceil(t.period) * t.wcet)
                .sum();
            let next = ti.wcet + interference;
            if next == r {
                return if r <= deadline { Some(r) } else { None };
            }
            if next > deadline {
                return None; // diverges past the deadline → not schedulable
            }
            r = next;
        }
    }

    /// Response times of all tasks (index-parallel to `tasks`).
    #[must_use]
    pub fn response_times(&self) -> Vec<Option<u64>> {
        (0..self.tasks.len())
            .map(|i| self.response_time(i))
            .collect()
    }

    /// Exact schedulability test (response-time analysis): all `R_i ≤ D_i`.
    #[must_use]
    pub fn is_schedulable_rta(&self) -> bool {
        (0..self.tasks.len()).all(|i| self.response_time(i).is_some())
    }

    /// Sufficient schedulability test (Liu & Layland): `U ≤ n·(2^(1/n) − 1)`.
    /// Only meaningful for rate-monotonic with implicit deadlines; a `false`
    /// does *not necessarily* mean not-schedulable (use RTA in that case).
    #[must_use]
    pub fn is_schedulable_ll(&self) -> bool {
        self.utilization() <= Self::liu_layland_bound(self.tasks.len()) + 1e-9
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::float_cmp)]
mod tests {
    use super::*;

    fn p(v: i16) -> Priority {
        Priority::new(v).unwrap()
    }

    #[test]
    fn implicit_deadline_defaults_to_period() {
        let t = Task::new(10, 3, p(5));
        assert_eq!(t.effective_deadline(), 10);
        let t2 = Task::with_deadline(10, 3, 7, p(5));
        assert_eq!(t2.effective_deadline(), 7);
    }

    #[test]
    fn rate_monotonic_assigns_shortest_period_highest() {
        let mut ts = TaskSet::new();
        ts.push(Task::new(20, 5, p(0)));
        ts.push(Task::new(4, 1, p(0)));
        ts.push(Task::new(5, 2, p(0)));
        ts.assign_rate_monotonic();
        // T=4 most urgent → highest priority; T=20 lowest.
        assert!(ts.tasks[1].priority > ts.tasks[2].priority);
        assert!(ts.tasks[2].priority > ts.tasks[0].priority);
    }

    #[test]
    fn response_time_analysis_exact_values() {
        // Classic RM set: (T,C) = (4,1),(5,2),(20,5).
        let mut ts = TaskSet::new();
        ts.push(Task::new(4, 1, p(0)));
        ts.push(Task::new(5, 2, p(0)));
        ts.push(Task::new(20, 5, p(0)));
        ts.assign_rate_monotonic();
        let r = ts.response_times();
        assert_eq!(r[0], Some(1)); // highest priority, no interference
        assert_eq!(r[1], Some(3)); // 2 + ⌈3/4⌉·1
        assert_eq!(r[2], Some(15)); // fixed point at 15
        assert!(ts.is_schedulable_rta());
    }

    #[test]
    fn rta_stronger_than_liu_layland() {
        // U = 0.25+0.4+0.25 = 0.9 > LL bound(3) ≈ 0.78 → LL says "no",
        // but RTA proves schedulability.
        let mut ts = TaskSet::new();
        ts.push(Task::new(4, 1, p(0)));
        ts.push(Task::new(5, 2, p(0)));
        ts.push(Task::new(20, 5, p(0)));
        ts.assign_rate_monotonic();
        assert!((ts.utilization() - 0.9).abs() < 1e-9);
        assert!(!ts.is_schedulable_ll());
        assert!(ts.is_schedulable_rta());
    }

    #[test]
    fn overloaded_set_is_unschedulable() {
        // (T,C) = (4,3),(5,3): U = 0.75+0.6 = 1.35 > 1 → impossible.
        let mut ts = TaskSet::new();
        ts.push(Task::new(4, 3, p(0)));
        ts.push(Task::new(5, 3, p(0)));
        ts.assign_rate_monotonic();
        assert!(!ts.is_schedulable_rta());
        assert!(ts.response_time(1).is_none());
    }

    #[test]
    fn liu_layland_bound_known_points() {
        assert!((TaskSet::liu_layland_bound(1) - 1.0).abs() < 1e-9);
        assert!((TaskSet::liu_layland_bound(2) - 0.8284271).abs() < 1e-6);
        assert!((TaskSet::liu_layland_bound(3) - 0.7797631).abs() < 1e-6);
        // Converges towards ln(2).
        let big = TaskSet::liu_layland_bound(1000);
        assert!((big - core::f64::consts::LN_2).abs() < 1e-3, "big={big}");
    }

    #[test]
    fn constrained_deadline_tightens_test() {
        // (4,1),(5,2),(20,5) is schedulable with D=T; R_3 = 15.
        // With D_3 = 12 < 15, task 3 becomes unschedulable.
        let mut ts = TaskSet::new();
        ts.push(Task::new(4, 1, p(0)));
        ts.push(Task::new(5, 2, p(0)));
        ts.push(Task::with_deadline(20, 5, 12, p(0)));
        ts.assign_rate_monotonic();
        assert!(!ts.is_schedulable_rta());
    }
}
