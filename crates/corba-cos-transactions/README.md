# zerodds-corba-cos-transactions

OMG **Object Transaction Service** (OTS / `CosTransactions`) 1.4 — pure-Rust
`no_std + alloc`, `forbid(unsafe_code)`.

Distributed transactions (ACID) for the CORBA stack — the core lever for the
"drop-in for legacy finance systems" migration.

- **`otid_t`** — cross-ORB transaction identifier (byte-exact CDR codec).
- **`PropagationContext`** + `TransactionService` service context (id = 0) —
  transaction-context propagation over GIOP.
- **2-phase commit** — `prepare`/`commit`/`rollback`/`commit_one_phase`/`forget`
  with a vote-driven state machine, read-only optimization + one-phase commit.
- **`Current` / `Coordinator` / `Terminator` / `Control`** — the OTS
  orchestration interfaces.

```rust
use zerodds_corba_cos_transactions::{Current, Vote};

let mut current = Current::new(30); // 30s Timeout
current.begin().unwrap();
let coord = current.get_control().unwrap().get_coordinator();
coord.register_resource(debit).unwrap();   // impl Resource
coord.register_resource(credit).unwrap();
current.commit().unwrap();                 // treibt 2-Phase-Commit
```

Spec: OMG Transaction Service 1.4. RT-CORBA is covered via the DDS-QoS side;
CosTrading is subordinate.
