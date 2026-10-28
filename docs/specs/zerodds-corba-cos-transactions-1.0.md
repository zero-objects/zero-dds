# `zerodds-corba-cos-transactions` v1.0 — OTS: Object Transaction Service in Rust

ZeroDDS vendor spec. Implemented in `crates/corba-cos-transactions`. Authored in
the style of the OMG CORBA spec (numbered clauses, RFC 2119 keywords,
conformance profile). The **wire formats** (`otid_t`, `PropagationContext`,
`TransactionService` ServiceContext) are OMG-normative (OMG Transaction Service
1.4); this spec standardises the **ZeroDDS-own Rust PSM** of this service — the
OMG has not standardised a Rust language mapping (cf. `01_scope_and_specs.md`,
"ZeroDDS-own part, not OMG-standardized").

## Motivation

The CORBA installed base in the financial industry depends on the **Object Transaction Service**:
distributed transactions with 2-phase commit across several ORBs, propagated via the
GIOP `TransactionService` ServiceContext. Without OTS, a financial migration to
a new ORB is not "drop-in". `zerodds-corba-cos-transactions` delivers the OTS
core in `no_std + alloc`, `forbid(unsafe_code)`, with byte-conformant wire
propagation against JacORB as the reference ORB.

## Goals

- **`otid_t` wire byte-conformant** to OMG OTS App. A — the cross-ORB
  transaction identity, verified against JacORB `otid_tHelper`.
- **`PropagationContext` over GIOP** in the `TransactionService` ServiceContext
  (id = 0), byte-conformant to JacORB `PropagationContextHelper`.
- **2-phase commit engine** with a vote-driven state machine, read-only and
  one-phase optimisation, heuristic handling.
- **Idiomatic Rust PSM**: `Otid`, `PropagationContext`, `Resource`, `Vote`,
  `Current`/`Coordinator`/`Terminator`/`Control`.

## Non-goals

- **Recoverable 2PC with a persistent log** (recovery after a crash) — v1.0 is
  in-memory; durability is the responsibility of the `Resource` implementation.
- **Nested transactions with sub-coordinator federation** — the
  `PropagationContext` carries `parents`, but the sub-transaction semantics (§OTS
  10.4) are only structural in v1.0, not fully orchestrated.
- **Live coordinator/terminator object ref callback** over GIOP (distributed 2PC
  over real IORs) — the `*_ior` fields are carried (nil for a flat tx);
  the typed embedding is a follow-up extension.

## §1 `otid_t` — transaction identity

### §1.1 Structure

```text
struct otid_t {
    long             formatID;        // X/Open XID format (-1 = NULL)
    long             bequeath_length; // tid prefix inherited by a sub-tx
    sequence<octet>  tid;             // global transaction id + branch qualifier
};
```

The Rust PSM is `Otid { format_id: i32, bequeath_length: i32, tid: Vec<u8> }`.
`Otid::new(format_id, tid)` sets `bequeath_length` to the full `tid` length
(root transaction); `Otid::null()` returns `formatID = -1`.

### §1.2 CDR encoding

`Otid::encode`/`decode` MUST write/read exactly `i32 formatID`, `i32 bequeath_length`,
`u32 tid.len()`, `tid` bytes. Big- and little-endian.

**Byte conformance (normative).** `otid_t(formatID=7, bequeath_length=3,
tid=[0xAA,0xBB,0xCC])` MUST encode big-endian to

```
00000007 00000003 00000003 aabbcc
```

— byte-identical to JacORB 3.9 `org.omg.CosTransactions.otid_tHelper.write`.

## §2 `PropagationContext` — transaction context propagation

### §2.1 ServiceContext

A transactional request carries in the IOP `ServiceContext` with the id
`TRANSACTION_SERVICE_CONTEXT_ID` (= 0, `TransactionService`) a CDR
encapsulation (byte order octet + body) of the `PropagationContext`:

```text
struct TransIdentity {
    Coordinator coord;   // object reference (nil ref: type_id "" + 0 profiles)
    Terminator  term;    // object reference
    otid_t      otid;
};
struct PropagationContext {
    unsigned long           timeout;
    TransIdentity           current;
    sequence<TransIdentity> parents;
    any                     implementation_specific_data;  // tk_null
};
```

### §2.2 Object reference encoding

`coord`/`term` MUST be encoded as CORBA object references. A
**nil reference** is `type_id ""` (CDR length 1 + NUL) + 0 TaggedProfiles. The
`implementation_specific_data` `any` is `tk_null` (1 word `00000000`).

**Byte conformance (normative).** `PropagationContext(timeout=30,
current={nil coord, nil term, otid(0,4,[0,0,0,1])}, parents=[], tk_null)` MUST
encode big-endian to

```
0000001e 00000001 00000000 00000000 00000001 00000000 00000000
00000000 00000004 00000004 00000001 00000000 00000000
```

(52 bytes) — byte-identical to JacORB 3.9
`org.omg.CosTransactions.PropagationContextHelper.write`.

### §2.3 API

`PropagationContext::flat(timeout, otid)` builds the flat context;
`to_service_context_data(endianness)` / `from_service_context_data(bytes)`
serialise the encapsulation.

## §3 2-phase commit

### §3.1 `Resource` + `Vote`

A `Resource` is a transactional participant (OMG OTS §10.3.2):

```rust
pub trait Resource {
    fn prepare(&self) -> Vote;                          // Phase 1
    fn commit(&self) -> Result<(), HeuristicOutcome>;   // Phase 2
    fn rollback(&self) -> Result<(), HeuristicOutcome>;
    fn commit_one_phase(&self) -> Result<(), HeuristicOutcome>;
    fn forget(&self);
}
```

`Vote` is `Commit` | `Rollback` | `ReadOnly`.

### §3.2 Coordinator algorithm

`coordinate_commit(resources)` MUST:

1. **empty set** → `Committed` (nothing to do),
2. **one resource** → `commit_one_phase` (one-phase optimisation),
3. **otherwise** phase 1 (`prepare` all): if *one* votes `Rollback` → `rollback` all resources
   already prepared with `Commit`, result `RolledBack`;
   otherwise phase 2 → `commit` all `Commit` voters (`ReadOnly` is dropped),
   result `Committed` (or `HeuristicMixed` on a heuristic deviation).

### §3.3 Orchestration

`Current` (§OTS 10.3.6) is context-bound: `begin` creates a transaction
with a fresh `otid` (counter-based), `commit`/`rollback` end it via the
`Terminator`. `rollback_only` marks the transaction; a subsequent `commit`
MUST then return `RolledBack`. `Coordinator::register_resource` is only allowed in
`Active` status.

## §4 Conformance

An **OTS-conformant** ZeroDDS module:

1. encodes `otid_t` and `PropagationContext` byte-conformant according to §1.2/§2.2,
2. propagates the context in the ServiceContext id = 0 (§2.1),
3. implements the 2PC according to §3.2 incl. read-only and one-phase optimisation,
4. delivers the `Current`/`Coordinator`/`Terminator` semantics according to §3.3.

## §5 Implementation mapping

| Spec | Code |
|---|---|
| §1 `otid_t` | `corba-cos-transactions/src/otid.rs` — `Otid` |
| §2 PropagationContext | `corba-cos-transactions/src/propagation.rs` — `PropagationContext`, `TransIdentity`, `TRANSACTION_SERVICE_CONTEXT_ID` |
| §3 2-phase commit | `corba-cos-transactions/src/two_phase.rs` — `Vote`, `Resource`, `coordinate_commit` |
| §3.3 Orchestration | `corba-cos-transactions/src/transaction.rs` — `Current`, `Coordinator`, `Terminator`, `Control`, `Status` |

## §6 Tests

- Unit (24): `otid` roundtrip BE+LE + golden vector; `PropagationContext`
  ServiceContext roundtrip + JacORB byte conformance; 2PC state transitions
  (all-commit/veto/read-only/one-phase/heuristic); `Current` begin/commit/rollback.
- E2E: `tests/ots_distributed.rs` — atomic bank transfer across two resources,
  atomic rollback on veto, context propagation over the wire.
- Cross-ORB byte conformance: `otid_t` + `PropagationContext` byte-identical to
  JacORB 3.9 (`competitors/jacorb/csiv2-ots/Dump.java`).

## Annex A — Heuristics

A `Resource` may decide on its own authority (`HeuristicOutcome::Mixed`/
`Rollback`/`Commit`/`Hazard`). If a committed resource deviates from the coordinator
decision, the coordinator MUST report `HeuristicMixed` and allow the resource
to `forget`. Persistent heuristic logging is a non-goal (§Non-goals).
