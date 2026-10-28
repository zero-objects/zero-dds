# `zerodds-corba-rt` v1.0 — Real-Time CORBA in Rust

ZeroDDS vendor spec. Implemented in `crates/corba-rt`. Authored in the style of the
OMG CORBA spec (numbered clauses, RFC-2119 keywords, conformance profile). The
**priority propagation wire** (`RTCorbaPriority` service context) is OMG-normative
(OMG Real-Time CORBA 1.0); this spec standardizes the **ZeroDDS-specific Rust PSM** of the
RT policies — the OMG has not standardized a Rust language mapping.

## Motivation

**Real-Time CORBA** brings deterministic real-time behaviour into the CORBA model:
end-to-end priority propagation, priority-aware thread pools, priority-banded
connections. It is TAO's flagship and widespread in the avionics/defence
installed base. `zerodds-corba-rt` provides the RT policy model + the
priority propagation in `no_std + alloc`, `forbid(unsafe_code)` — complementary
to the real-time QoS on the DDS side (deadline/latency budget/transport priority).

## Goals

- **CORBA priority** (`0..=32767`) + **PriorityMapping** ↔ native OS priority.
- **PriorityModel** `SERVER_DECLARED` / `CLIENT_PROPAGATED` + `PriorityModelPolicy`.
- **`RTCorbaPriority` service context** (id = 10) — propagation of the client priority
  over GIOP, byte-exact CDR.
- **Threadpool/lane** + **PriorityBand** with priority-based selection.
- **`RTCORBA::Current`** — CORBA priority of the current context.

## Non-goals

- **Native thread spawning + OS scheduler binding** — v1.0 models the
  threadpool/lane **structure + selection**; the actual spawning and
  `pthread_setschedparam` is a matter of the runtime integration.
- **Priority inheritance mutexes** (OMG RTCORBA §5.6 `Mutex`) — follow-up extension.
- **Static Scheduling Service** (RT-CORBA scheduling profile) — non-goal for v1.0.

## §1 Priority + mapping

### §1.1 `Priority`

A CORBA priority is a `short` in the range `0..=32767` (RT-CORBA §5.3);
higher = more urgent. `Priority::new(v)` MUST reject values outside it;
`Priority::clamped(v)` clamps.

### §1.2 `PriorityMapping`

```rust
pub trait PriorityMapping {
    fn to_native(&self, corba: Priority) -> Option<i32>;
    fn to_corba(&self, native: i32) -> Option<Priority>;
}
```

The default `LinearPriorityMapping::new(native_min, native_max)` scales linearly
to a native window (e.g. POSIX `SCHED_FIFO` `1..99`). `to_corba` MUST reject native
values outside the window with `None`.

## §2 Priority model

### §2.1 `PriorityModelPolicy`

```rust
pub enum PriorityModel { ServerDeclared, ClientPropagated }
pub struct PriorityModelPolicy { pub model: PriorityModel, pub server_priority: Priority }
```

`effective_priority(propagated)` MUST return: for `ClientPropagated` the propagated
client priority (fallback `server_priority`), for `ServerDeclared` always
`server_priority`. The policy is announced in the IOR as `TAG_RT_CORBA_PRIORITY_MODEL`.

### §2.2 `RTCorbaPriority` service context

With `CLIENT_PROPAGATED` the client attaches its CORBA priority as an IOP
`ServiceContext` with the id `RT_CORBA_PRIORITY_SC_ID` (= 10, RT-CORBA §5.4.2)
— a CDR encapsulation (byte-order octet + `short`).

**Byte conformance (normative).** Priority `1337` big-endian MUST encode
to

```
00 000539
```

(BO octet 0, pad, `short` 0x0539). `encode_priority_context` /
`decode_priority_context` implement this.

## §3 Threadpools + bands

### §3.1 Threadpool/lane

```rust
pub struct Lane { pub priority: Priority, pub static_threads: u32, pub dynamic_threads: u32 }
pub struct Threadpool { pub lanes: Vec<Lane>, pub stacksize: usize,
                        pub allow_request_buffering: bool, pub max_buffered_requests: u32 }
```

`Threadpool::lane_for(priority)` MUST choose the lane with the **highest priority ≤
`priority`** (the most demanding one that still covers the request);
if there is none, the lowest lane.

### §3.2 PriorityBand

```rust
pub struct PriorityBand { pub low: Priority, pub high: Priority }
pub struct PriorityBandedConnectionPolicy { pub bands: Vec<PriorityBand> }
```

`band_for(priority)` MUST return the index of the band that covers `priority` —
the connection selection (prevents priority inversion on a shared connection).

## §4 `RTCORBA::Current`

`RtCurrent::get_priority`/`set_priority` (RT-CORBA §5.5) read/set the
CORBA priority of the current context. In a runtime integration
`set_priority` changes the native thread priority via the active `PriorityMapping`.

## §5 Conformance

An **RT-CORBA-conformant** ZeroDDS module:

1. validates/maps priorities according to §1,
2. provides the `effective_priority` semantics according to §2.1 for both models,
3. encodes the `RTCorbaPriority` service context byte-exactly according to §2.2 (id = 10),
4. selects lane/band according to §3,
5. provides `RTCORBA::Current` according to §4.

## §6 Implementation mapping

| Spec | Code |
|---|---|
| §1 Priority + mapping | `corba-rt/src/priority.rs` — `Priority`, `LinearPriorityMapping` |
| §2 Priority model | `corba-rt/src/policy.rs` — `PriorityModel`, `PriorityModelPolicy` |
| §2.2 SC propagation | `corba-rt/src/propagation.rs` — `RT_CORBA_PRIORITY_SC_ID`, `encode/decode_priority_context` |
| §3 Threadpool/bands | `corba-rt/src/policy.rs` — `Lane`, `Threadpool`, `PriorityBand`, `PriorityBandedConnectionPolicy` |
| §4 Current | `corba-rt/src/current.rs` — `RtCurrent` |

## §7 Tests

- Unit (12): priority range + linear mapping end points + native window; both
  priority models (`effective_priority`); lane selection + capacity; band selection;
  `RTCorbaPriority` SC round trip BE+LE + byte-exact golden; `RtCurrent` get/set.

## Annex A — Relation to the DDS real-time side

RT-CORBA and the DDS QoS real-time side are complementary: RT-CORBA propagates
CORBA priorities end-to-end over GIOP, DDS covers deadline/latency budget/
transport priority on the pub/sub side. A CCM component with GIOP ports and
DDS topic ports can use both models at the same time.
