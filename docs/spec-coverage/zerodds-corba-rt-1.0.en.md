# zerodds-corba-rt 1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-corba-rt-1.0.md` (ZeroDDS vendor spec)

Implementation:

- `crates/corba-rt/` — Real-Time CORBA.

## §1 Priority + mapping

### §1.1 `Priority`

**Spec:** §1.1 — CORBA priority `0..=32767`; `new` rejects out-of-range, `clamped` clamps.

**Repo:** `crates/corba-rt/src/priority.rs::Priority` (`new`, `clamped`, `value`).

**Tests:** `crates/corba-rt/src/priority.rs::tests::priority_range`.

**Status:** done

### §1.2 `PriorityMapping`

**Spec:** §1.2 — trait `to_native`/`to_corba`; `LinearPriorityMapping` linear onto the native window; native out-of-window → `None`.

**Repo:** `crates/corba-rt/src/priority.rs::PriorityMapping`, `LinearPriorityMapping`.

**Tests:** `priority.rs::tests::linear_mapping_endpoints`, `native_out_of_window_unmapped`.

**Status:** done

## §2 Priority model

### §2.1 `PriorityModelPolicy`

**Spec:** §2.1 — `PriorityModel` ServerDeclared/ClientPropagated; `effective_priority` (propagated vs server-declared).

**Repo:** `crates/corba-rt/src/policy.rs::PriorityModel`, `PriorityModelPolicy::effective_priority`.

**Tests:** `policy.rs::tests::client_propagated_uses_propagated_priority`, `server_declared_ignores_propagated`.

**Status:** done

### §2.2 `RTCorbaPriority` ServiceContext + byte conformance

**Spec:** §2.2 — client priority as ServiceContext id = 10, CDR encapsulation; BE golden `00 000539` (priority 1337).

**Repo:** `crates/corba-rt/src/propagation.rs::RT_CORBA_PRIORITY_SC_ID`, `encode_priority_context` / `decode_priority_context`.

**Tests:** `propagation.rs::tests::sc_id_is_ten`, `priority_context_roundtrip`, `byte_exact_big_endian`.

**Status:** done

## §3 Threadpools + bands

### §3.1 Threadpool/lane

**Spec:** §3.1 — `Lane`/`Threadpool`; `lane_for(priority)` = highest priority ≤ priority (otherwise lowest).

**Repo:** `crates/corba-rt/src/policy.rs::Lane`, `Threadpool::lane_for` + `static_capacity`.

**Tests:** `policy.rs::tests::lane_selection_picks_highest_covering`.

**Status:** done

### §3.2 PriorityBand

**Spec:** §3.2 — `PriorityBand`/`PriorityBandedConnectionPolicy`; `band_for(priority)` = index of the covering band.

**Repo:** `crates/corba-rt/src/policy.rs::PriorityBand`, `PriorityBandedConnectionPolicy::band_for`.

**Tests:** `policy.rs::tests::priority_band_selection`.

**Status:** done

## §4 `RTCORBA::Current`

### §4.1 RtCurrent

**Spec:** §4 — `RtCurrent::get_priority`/`set_priority` for the current context.

**Repo:** `crates/corba-rt/src/current.rs::RtCurrent`.

**Tests:** `current.rs::tests::get_set_priority`, `default_is_zero`.

**Status:** done

## §5 Threadpool runtime + mutex + scheduling

### §5.7 Threadpool runtime (lanes on OS threads)

**Spec:** §5.7 — the threadpool-with-lanes as a runnable runtime: OS threads per lane (static + dynamic), job routing by priority, native scheduler priority per lane.

**Repo:** `crates/corba-rt/src/threadpool.rs::ThreadpoolRuntime` (`start`/`dispatch`/`select_lane`), `NativePrioritySetter` hook (platform `unsafe` at the caller; the crate stays `forbid(unsafe_code)`).

**Tests:** `threadpool.rs::tests`: `dispatches_and_runs_all_jobs`, `routes_to_lane_by_priority`, `native_priority_hook_invoked_per_worker`, `rejects_when_buffering_off_and_no_worker`, `dynamic_worker_spawns_under_saturation`.

**Status:** done

### §5.6 `RTCORBA::Mutex` with priority inheritance

**Spec:** §5.6 — mutex with priority inheritance against priority inversion; the owner inherits the priority of the highest waiter.

**Repo:** `crates/corba-rt/src/mutex.rs::PriorityInheritance` (protocol core, `no_std`) + `RtMutex` (std, priority-ordered granting, event-driven via Condvar).

**Tests:** `mutex.rs::tests`: `owner_inherits_highest_waiter`, `priority_reverts_on_unblock`, `rt_mutex_basic_lock_unlock`, `rt_mutex_try_lock_contended`, `rt_mutex_grants_highest_priority_waiter_first`.

**Status:** done

### §Scheduling profile — static schedulability analysis

**Spec:** RT-CORBA scheduling profile — a-priori schedulability for fixed-priority preemptive scheduling (Rate-Monotonic + Response-Time Analysis).

**Repo:** `crates/corba-rt/src/scheduling.rs::TaskSet` (`assign_rate_monotonic`, `liu_layland_bound`, `response_time`/`is_schedulable_rta`).

**Tests:** `scheduling.rs::tests`: `response_time_analysis_exact_values`, `rta_stronger_than_liu_layland`, `overloaded_set_is_unschedulable`, `liu_layland_bound_known_points`, `constrained_deadline_tightens_test`, `rate_monotonic_assigns_shortest_period_highest`.

**Status:** done

---

## Audit status

10 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-corba-rt` — 30 tests green, 0 failed.
