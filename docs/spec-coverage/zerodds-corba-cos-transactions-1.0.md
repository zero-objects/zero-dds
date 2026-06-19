# zerodds-corba-cos-transactions 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-corba-cos-transactions-1.0.md` (ZeroDDS Vendor-Spec)

Implementation:

- `crates/corba-cos-transactions/` — CORBA Object Transaction Service (OTS).

## §1 `otid_t` — Transaktions-Identität

### §1.1 Struktur

**Spec:** §1.1 — `otid_t { long formatID; long bequeath_length; sequence<octet> tid; }`; Rust-PSM `Otid` mit `new`/`null`.

**Repo:** `crates/corba-cos-transactions/src/otid.rs::Otid` (`new`, `null`, `is_null`).

**Tests:** `crates/corba-cos-transactions/src/otid.rs::tests::null_otid`.

**Status:** done

### §1.2 CDR-Encoding + Byte-Konformität

**Spec:** §1.2 — `encode`/`decode` schreiben formatID, bequeath_length, tid-len, tid; Big-Endian-Golden `00000007 00000003 00000003 aabbcc` byte-identisch zu JacORB `otid_tHelper`.

**Repo:** `crates/corba-cos-transactions/src/otid.rs::Otid::encode` / `decode`.

**Tests:** `crates/corba-cos-transactions/src/otid.rs::tests::roundtrip_be_le`, `byte_exact_golden_be`.

**Status:** done

## §2 PropagationContext

### §2.1 ServiceContext (id = 0)

**Spec:** §2.1 — `PropagationContext` als CDR-Encapsulation im `TransactionService`-ServiceContext (id = 0).

**Repo:** `crates/corba-cos-transactions/src/propagation.rs::PropagationContext`, `TRANSACTION_SERVICE_CONTEXT_ID`, `to_service_context_data` / `from_service_context_data`.

**Tests:** `crates/corba-cos-transactions/src/propagation.rs::tests::service_context_id_is_zero`, `flat_context_service_context_roundtrip`.

**Status:** done

### §2.2 Object-Reference-Encoding + Byte-Konformität

**Spec:** §2.2 — coord/term als Object-References (nil = type_id "" + 0 Profile); `implementation_specific_data` = tk_null (1 Wort); 52-Byte-Golden byte-identisch zu JacORB `PropagationContextHelper`.

**Repo:** `crates/corba-cos-transactions/src/propagation.rs` (`write_object_ref` / `read_object_ref`, `TransIdentity`).

**Tests:** `crates/corba-cos-transactions/src/propagation.rs::tests::propagation_context_byte_identical_to_jacorb`, `trans_identity_nil_refs_roundtrip`, `nested_context_with_parents`.

**Status:** done

### §2.3 API

**Spec:** §2.3 — `flat(timeout, otid)` + Encapsulation-Serialisierung.

**Repo:** `crates/corba-cos-transactions/src/propagation.rs::PropagationContext::flat`.

**Tests:** `tests/ots_distributed.rs::transaction_context_propagates_over_wire`.

**Status:** done

## §3 2-Phase-Commit

### §3.1 `Resource` + `Vote`

**Spec:** §3.1 — `Resource`-Trait (prepare/commit/rollback/commit_one_phase/forget) + `Vote` (Commit/Rollback/ReadOnly).

**Repo:** `crates/corba-cos-transactions/src/two_phase.rs::Resource`, `Vote`, `HeuristicOutcome`.

**Tests:** `crates/corba-cos-transactions/src/two_phase.rs::tests::all_commit_commits_everyone`.

**Status:** done

### §3.2 Coordinator-Algorithmus

**Spec:** §3.2 — leere Menge → Committed; eine Resource → One-Phase; sonst Prepare-Phase, dann Commit/Rollback je Votes; Read-Only entfällt in Phase 2.

**Repo:** `crates/corba-cos-transactions/src/two_phase.rs::coordinate_commit` + `coordinate_rollback`.

**Tests:** `two_phase.rs::tests`: `one_rollback_vote_rolls_back_prepared`, `read_only_skips_phase_two`, `single_resource_uses_one_phase`, `single_resource_one_phase_rollback`, `empty_set_commits_trivially`, `heuristic_commit_failure_reports_mixed`.

**Status:** done

### §3.3 Orchestrierung

**Spec:** §3.3 — `Current` (begin/commit/rollback, frische otid), `rollback_only` → folgendes commit liefert RolledBack; `register_resource` nur im Active-Status.

**Repo:** `crates/corba-cos-transactions/src/transaction.rs::Current`, `Coordinator`, `Terminator`, `Control`, `Status`.

**Tests:** `transaction.rs::tests`: `current_begin_commit_two_resources`, `rollback_only_forces_rollback_on_commit`, `explicit_rollback`, `commit_without_transaction_errors`, `double_begin_rejected`, `suspend_resume_keeps_resources`, `propagation_context_carries_otid`.

**Status:** done

## Annex A — Heuristik

### A.1 Heuristic-Outcome

**Spec:** Annex A — `HeuristicOutcome` (Mixed/Rollback/Commit/Hazard); Abweichung → Coordinator meldet `HeuristicMixed`, Resource darf `forget`.

**Repo:** `crates/corba-cos-transactions/src/two_phase.rs::HeuristicOutcome` + `two_phase`-Pfad.

**Tests:** `two_phase.rs::tests::heuristic_commit_failure_reports_mixed`.

**Status:** done

### A.2 Durability — Transaction-Log + RecoveryCoordinator (§10.3.7)

**Spec:** §10.3.7 `RecoveryCoordinator` — crash-recoverable 2PC: die Commit-Entscheidung wird vor Phase 2 force-geschrieben; ein Neustart löst in-doubt-Transaktionen auf (presumed-abort).

**Repo:** `crates/corba-cos-transactions/src/recovery.rs`: `TransactionLog`-Trait + `InMemoryLog`; `coordinate_commit_durable` (force-write `Decided` vor Phase 2); `RecoveryCoordinator` (`recover`/`replay_completion` mit presumed-abort) + `ResourceResolver`.

**Tests:** `recovery.rs::tests` (8): `durable_commit_logs_decision_then_completion`, `recovery_completes_in_doubt_commit`/`_rollback`, `replay_completion_returns_logged_decision`, `replay_completion_presumed_abort_for_unknown`, `preparing_state_is_presumed_abort`, `single_resource_durable_one_phase`, `durable_rollback_decision_logged`.

**Status:** done

---

## Audit-Status

10 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-corba-cos-transactions` — 30 Unit + 3 e2e grün, 0 failed.
