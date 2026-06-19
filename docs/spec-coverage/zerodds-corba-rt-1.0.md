# zerodds-corba-rt 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-corba-rt-1.0.md` (ZeroDDS Vendor-Spec)

Implementation:

- `crates/corba-rt/` — Real-Time CORBA.

## §1 Priorität + Mapping

### §1.1 `Priority`

**Spec:** §1.1 — CORBA-Priorität `0..=32767`; `new` lehnt außerhalb ab, `clamped` klemmt.

**Repo:** `crates/corba-rt/src/priority.rs::Priority` (`new`, `clamped`, `value`).

**Tests:** `crates/corba-rt/src/priority.rs::tests::priority_range`.

**Status:** done

### §1.2 `PriorityMapping`

**Spec:** §1.2 — Trait `to_native`/`to_corba`; `LinearPriorityMapping` linear auf natives Fenster; native außerhalb → `None`.

**Repo:** `crates/corba-rt/src/priority.rs::PriorityMapping`, `LinearPriorityMapping`.

**Tests:** `priority.rs::tests::linear_mapping_endpoints`, `native_out_of_window_unmapped`.

**Status:** done

## §2 Priority-Model

### §2.1 `PriorityModelPolicy`

**Spec:** §2.1 — `PriorityModel` ServerDeclared/ClientPropagated; `effective_priority` (propagiert vs server-deklariert).

**Repo:** `crates/corba-rt/src/policy.rs::PriorityModel`, `PriorityModelPolicy::effective_priority`.

**Tests:** `policy.rs::tests::client_propagated_uses_propagated_priority`, `server_declared_ignores_propagated`.

**Status:** done

### §2.2 `RTCorbaPriority`-ServiceContext + Byte-Konformität

**Spec:** §2.2 — Client-Priorität als ServiceContext id = 10, CDR-Encapsulation; BE-Golden `00 000539` (Priorität 1337).

**Repo:** `crates/corba-rt/src/propagation.rs::RT_CORBA_PRIORITY_SC_ID`, `encode_priority_context` / `decode_priority_context`.

**Tests:** `propagation.rs::tests::sc_id_is_ten`, `priority_context_roundtrip`, `byte_exact_big_endian`.

**Status:** done

## §3 Threadpools + Bänder

### §3.1 Threadpool/Lane

**Spec:** §3.1 — `Lane`/`Threadpool`; `lane_for(priority)` = höchste Priorität ≤ priority (sonst niedrigste).

**Repo:** `crates/corba-rt/src/policy.rs::Lane`, `Threadpool::lane_for` + `static_capacity`.

**Tests:** `policy.rs::tests::lane_selection_picks_highest_covering`.

**Status:** done

### §3.2 PriorityBand

**Spec:** §3.2 — `PriorityBand`/`PriorityBandedConnectionPolicy`; `band_for(priority)` = Index des abdeckenden Bandes.

**Repo:** `crates/corba-rt/src/policy.rs::PriorityBand`, `PriorityBandedConnectionPolicy::band_for`.

**Tests:** `policy.rs::tests::priority_band_selection`.

**Status:** done

## §4 `RTCORBA::Current`

### §4.1 RtCurrent

**Spec:** §4 — `RtCurrent::get_priority`/`set_priority` für den aktuellen Kontext.

**Repo:** `crates/corba-rt/src/current.rs::RtCurrent`.

**Tests:** `current.rs::tests::get_set_priority`, `default_is_zero`.

**Status:** done

## §5 Threadpool-Runtime + Mutex + Scheduling

### §5.7 Threadpool-Runtime (Lanes auf OS-Threads)

**Spec:** §5.7 — der Threadpool-mit-Lanes als lauffähige Laufzeit: pro Lane OS-Threads (statisch + dynamisch), Job-Routing nach Priorität, native Scheduler-Priorität pro Lane.

**Repo:** `crates/corba-rt/src/threadpool.rs::ThreadpoolRuntime` (`start`/`dispatch`/`select_lane`), `NativePrioritySetter`-Hook (plattform-`unsafe` beim Aufrufer, Crate bleibt `forbid(unsafe_code)`).

**Tests:** `threadpool.rs::tests`: `dispatches_and_runs_all_jobs`, `routes_to_lane_by_priority`, `native_priority_hook_invoked_per_worker`, `rejects_when_buffering_off_and_no_worker`, `dynamic_worker_spawns_under_saturation`.

**Status:** done

### §5.6 `RTCORBA::Mutex` mit Priority-Inheritance

**Spec:** §5.6 — Mutex mit Priority-Inheritance gegen Priority-Inversion; Owner erbt die Priorität des höchsten Waiters.

**Repo:** `crates/corba-rt/src/mutex.rs::PriorityInheritance` (Protokollkern, `no_std`) + `RtMutex` (std, prioritäts-geordnete Vergabe, event-driven via Condvar).

**Tests:** `mutex.rs::tests`: `owner_inherits_highest_waiter`, `priority_reverts_on_unblock`, `rt_mutex_basic_lock_unlock`, `rt_mutex_try_lock_contended`, `rt_mutex_grants_highest_priority_waiter_first`.

**Status:** done

### §Scheduling-Profil — Statische Schedulability-Analyse

**Spec:** RT-CORBA-Scheduling-Profil — a-priori-Schedulability für fixed-priority preemptives Scheduling (Rate-Monotonic + Response-Time-Analyse).

**Repo:** `crates/corba-rt/src/scheduling.rs::TaskSet` (`assign_rate_monotonic`, `liu_layland_bound`, `response_time`/`is_schedulable_rta`).

**Tests:** `scheduling.rs::tests`: `response_time_analysis_exact_values`, `rta_stronger_than_liu_layland`, `overloaded_set_is_unschedulable`, `liu_layland_bound_known_points`, `constrained_deadline_tightens_test`, `rate_monotonic_assigns_shortest_period_highest`.

**Status:** done

---

## Audit-Status

10 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-corba-rt` — 30 Tests grün, 0 failed.
