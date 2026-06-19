# zerodds-corba-rt

OMG **Real-Time CORBA** 1.0 — pure-Rust `no_std + alloc`, `forbid(unsafe_code)`.

Deterministic real-time behavior for legacy CORBA code: end-to-end
priority propagation, priority-aware thread pools, priority-banded
connections.

- **`Priority`** + **`PriorityMapping`** — CORBA priority (0..32767) ↔ native OS priority.
- **`PriorityModel`** — `SERVER_DECLARED` vs `CLIENT_PROPAGATED` + `PriorityModelPolicy`.
- **`Threadpool`/`Lane`** + **`PriorityBand`** — priority-based lane/connection selection.
- **`RTCorbaPriority` ServiceContext** (id 10) — propagation of the client priority over GIOP.
- **`RtCurrent`** — CORBA priority of the current context.

```rust
use zerodds_corba_rt::{Priority, PriorityModel, PriorityModelPolicy};

let policy = PriorityModelPolicy {
    model: PriorityModel::ClientPropagated,
    server_priority: Priority::new(10).unwrap(),
};
// Client priority applies at the server:
let eff = policy.effective_priority(Some(Priority::new(99).unwrap()));
```

Spec: OMG Real-Time CORBA 1.0. Complementary to the DDS real-time QoS side
(Deadline/Latency-Budget/Transport-Priority).
