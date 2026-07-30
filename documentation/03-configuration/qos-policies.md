# QoS policies

DDS QoS is the contract between a DataWriter and a DataReader.
Per OMG DDS 1.4 §2.2.3, a publisher *offers* QoS and a subscriber
*requests* QoS; they match if `offered ⊇ requested`.

ZeroDDS implements every QoS policy from the DDS 1.4 spec.

## Reliability — `zerodds_qos::Reliability` / `ReliabilityKind`

| Kind | Behavior |
|---|---|
| `BestEffort` | Sample is sent once; loss = drop. Lowest latency. |
| `Reliable` | Acknowledged-and-retransmitted until matched. Default for tools that need lossless delivery. |

Match rule: writer `Reliable` ⊇ reader `Reliable | BestEffort`,
writer `BestEffort` ⊇ only reader `BestEffort`.

## Durability — `DurabilityKind`

| Kind | Behavior |
|---|---|
| `Volatile` | No replay for late-joining readers. Default. |
| `TransientLocal` | Writer keeps history, late-joiners replay matched samples. |
| `Transient` | Like `TransientLocal` but persisted to a vendor-managed transient service. ZeroDDS treats it as TransientLocal for now. |
| `Persistent` | Survives writer restart. Persistence backend pluggable; not all backends ship today. |

## History — `History`

| Kind | Behavior |
|---|---|
| `KeepLast(N)` | Cache holds at most N samples; oldest is evicted on overflow. Default `KeepLast(1)`. |
| `KeepAll` | Cache grows unbounded — `Reliability=Reliable` is recommended; backed up by `ResourceLimits`. |

## Deadline — `DeadlineQosPolicy`

A writer offers a "I will write at least every D nanoseconds"
contract. The reader requests the same; if writer is slow, the
reader's `requested_deadline_missed_count` increments.
`Duration::ZERO` (the default) = infinite = no monitoring.

## Liveliness — `LivelinessQosPolicy`

| Kind | Semantics |
|---|---|
| `Automatic` | Each write counts as an alive-assertion. Default. |
| `ManualByParticipant` | One participant-level periodic assertion; covers all its writers. |
| `ManualByTopic` | Per-writer explicit `assert_liveliness()` call. |

`lease_duration` is how long after the last assertion before the
peer is declared "not alive". Default infinite.

## Lifespan — `LifespanQosPolicy`

A sample becomes invisible to *future* readers after `duration`
nanoseconds; existing readers may still hold it. `Duration::ZERO` =
infinite. Useful for telemetry where stale data is misleading.

## Ownership — `OwnershipKind`

| Kind | Behavior |
|---|---|
| `Shared` | All writers visible. Default. |
| `Exclusive` | For each instance, only the highest-strength writer's samples reach readers. |

`ownership_strength` is an `i32` per writer. Higher wins.

## Partition — `partition: Vec<String>`

A writer publishes to one or more named partitions; subscribers
match if their partition list overlaps. Wildcards (`*`, `?`)
allowed. Empty list = default partition (`""`).

## Resource Limits — `ResourceLimitsQosPolicy`

| Field | Purpose |
|---|---|
| `max_samples` | Cap across all instances |
| `max_instances` | Cap on instance count |
| `max_samples_per_instance` | Per-instance cap |

Used for `KeepAll` history caches to bound memory.

## Time-Based Filter — `TimeBasedFilterQosPolicy`

Reader-side rate limiter: deliver at most one sample per
`minimum_separation` per instance. Useful for bandwidth-throttled
subscribers.

## Reader/Writer Data Lifecycle

| Policy | Effect |
|---|---|
| `WriterDataLifecycle.autodispose_unregistered_instances` | When the writer disposes an instance, also unregister it. Default `true`. |
| `ReaderDataLifecycle.autopurge_*_samples_delay` | When to purge no-longer-relevant samples on the reader. |

## Compatibility — what writer offered must cover what reader requested

ZeroDDS computes compatibility automatically in
`DcpsRuntime::wire_writer_to_remote_reader`. Mismatches bump
`offered_incompatible_qos.policies` on the writer side and
`requested_incompatible_qos.policies` on the reader side, with
the canonical OMG policy IDs (Spec §C.3, Table 5).

Per-policy bumping means each rejected policy is counted
individually; the listener can fire `on_offered_incompatible_qos`
once per policy mismatch.

## Setting QoS in code

```rust
use zerodds_dcps::runtime::UserWriterConfig;
use zerodds_qos::*;
use zerodds_types::{PrimitiveKind, TypeIdentifier};

let cfg = UserWriterConfig {
    topic_name: "Telemetry".into(),
    type_name: "Robot::Pose".into(),
    reliable: true,                                  // = Reliability::Reliable
    durability: DurabilityKind::TransientLocal,
    deadline: DeadlineQosPolicy {
        period: Duration::from_millis(50),
    },
    lifespan: LifespanQosPolicy {
        duration: Duration::from_secs(5),
    },
    liveliness: LivelinessQosPolicy {
        kind: LivelinessKind::Automatic,
        lease_duration: Duration::from_secs(2),
    },
    ownership: OwnershipKind::Exclusive,
    ownership_strength: 100,
    partition: vec!["sensor.*".into()],
    user_data: vec![],
    topic_data: vec![],
    group_data: vec![],
    type_identifier: TypeIdentifier::Primitive(PrimitiveKind::UInt8),
    data_representation_offer: None,
};
```

> ▶ Runnable example: [`qos-policies-full-writer`](https://github.com/zero-objects/zero-dds-snippets/tree/master/qos-policies-full-writer)
> (registers this exact config against a live `DcpsRuntime`).

## Reading further

- OMG DDS 1.4 §2.2.3 — full normative reference.
- `crates/qos/src/lib.rs` — every policy, with rustdoc explaining
  defaults and match rules.
