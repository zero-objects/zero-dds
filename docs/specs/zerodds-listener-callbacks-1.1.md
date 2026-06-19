# `zerodds-listener-callbacks` v1.1 — Vendor Spec

ZeroDDS vendor spec. Implemented in
`crates/zerodds-c-api/src/listener_ffi.rs`; the C struct definitions are
exported via cbindgen into `crates/zerodds-c-api/include/zerodds.h`.

## Motivation

The DDS spec (DDS 1.4 §2.2.4 *Listeners, Conditions, and Wait-sets*)
defines the listener concept normatively only for **language PSMs** that
support classes (DDS-PSM-Cxx 1.0 §7.5.9, DDS-Java-PSM 1.0 §8.7;
DDS-PSM-CSharp is not formally standardized). For **C** there is no
normative listener mapping path — RTI Connext, Eclipse Cyclone and
Fast-DDS each ship their own proprietary C listener APIs without
cross-vendor compatibility.

This spec defines a **complete C-FFI listener API** as a ZeroDDS vendor
extension. The API is:

- **A cross-language hub** consumed by `crates/cpp/`
  (`DataWriterListener`), `crates/cs/` (`IDataWriterListener`) and the
  other bindings.
- **NativeAOT-compatible:** no reflection, no GC allocations on the
  callback path.
- **Complete:** all listener callbacks from DDS 1.4 §2.2.4 Table 2.10 are
  exposed as C function-pointer slots, and every status callback is
  actively fired (see §6).

## Goals

- **Spec completeness:** every normative listener callback from DDS 1.4
  §2.2.4 has a C-FFI function-pointer slot.
- **Active delivery:** every status callback is fired on a real
  status-counter change, at the owning entity level and at every
  aggregator level bound to it.
- **Aggregation:** Publisher, Subscriber and DomainParticipant listeners
  aggregate the status of their contained entities.
- **Status-mask filter:** the caller selects which callbacks are active
  (DDS 1.4 §2.2.4.2.1.4).
- **An explicit thread-safety contract.**

## Non-Goals

- Generic listener containers (`AnyXxx` types): left to the language
  bindings.
- Synchronous listener calls out of `*_set_listener` (the DDS spec
  forbids this — "listener calls happen in implementation-specific
  threads").
- Listener chains (multiple listeners per entity): the per-entity slot is
  1:1, but the same listener struct may be bound to several entities
  (see §5).

## §1 Architecture

### §1.1 Function-pointer table (vtable)

Each entity type has its own `Zerodds*Listener` struct, declared
`#[repr(C)]` with function pointers. All pointers are optional
(NULL = callback ignored).

```c
typedef struct {
    void* user_data;                                        // §1.2
    void (*on_liveliness_lost)(void* user_data, zerodds_DataWriter* dw);
    void (*on_offered_deadline_missed)(void* user_data, zerodds_DataWriter* dw);
    void (*on_offered_incompatible_qos)(void* user_data, zerodds_DataWriter* dw);
    void (*on_publication_matched)(void* user_data, zerodds_DataWriter* dw);
} zerodds_DataWriterListener;
```

### §1.2 `user_data` slot

Each listener struct carries one `void* user_data` field. It is passed
unchanged to every callback. The caller wraps its state object in it
(for example a `GCHandle` for C#, a `PyObject*` for Python). Lifetime is
the caller's responsibility.

### §1.3 Set/Get API

```c
int zerodds_dp_set_listener(zerodds_DomainParticipant* p,
                            const zerodds_DomainParticipantListener* l,
                            uint32_t status_mask);
const zerodds_DomainParticipantListener* zerodds_dp_get_listener(
    zerodds_DomainParticipant* p);
```

One pair per entity type. A NULL pointer on `set_*` clears the slot and
the cached status counters for that entity.

## §2 Listener inventory (all 6 entity types)

### §2.1 DomainParticipantListener (DDS 1.4 §2.2.4.2.1)

```c
typedef struct {
    void* user_data;
    void (*on_inconsistent_topic)(void* user_data, zerodds_Topic* t);
    void (*on_data_on_readers)(void* user_data, zerodds_Subscriber* sub);
} zerodds_DomainParticipantListener;
```

The DomainParticipant listener handles the two genuinely
participant-level aggregate events (`INCONSISTENT_TOPIC` across the
participant's topics, `DATA_ON_READERS` across its subscribers).
Per-endpoint statuses are observed at the DataWriter/DataReader level or
via the Publisher/Subscriber aggregators (§5).

### §2.2 PublisherListener (DDS 1.4 §2.2.4.2.2)

```c
typedef struct {
    void* user_data;
    void (*on_offered_deadline_missed)(void*, zerodds_DataWriter*);
    void (*on_liveliness_lost)(void*, zerodds_DataWriter*);
    void (*on_offered_incompatible_qos)(void*, zerodds_DataWriter*);
    void (*on_publication_matched)(void*, zerodds_DataWriter*);
} zerodds_PublisherListener;
```

Aggregates the writer statuses of every DataWriter the publisher
contains.

### §2.3 SubscriberListener (DDS 1.4 §2.2.4.2.3)

```c
typedef struct {
    void* user_data;
    void (*on_data_on_readers)(void*, zerodds_Subscriber*);
    void (*on_sample_lost)(void*, zerodds_DataReader*);
    void (*on_sample_rejected)(void*, zerodds_DataReader*);
    void (*on_liveliness_changed)(void*, zerodds_DataReader*);
    void (*on_subscription_matched)(void*, zerodds_DataReader*);
    void (*on_requested_deadline_missed)(void*, zerodds_DataReader*);
    void (*on_requested_incompatible_qos)(void*, zerodds_DataReader*);
    void (*on_data_available)(void*, zerodds_DataReader*);
} zerodds_SubscriberListener;
```

Aggregates the reader statuses of every DataReader the subscriber
contains and fires `on_data_on_readers` with set semantics (§6.3).

### §2.4 TopicListener (DDS 1.4 §2.2.4.2.4)

```c
typedef struct {
    void* user_data;
    void (*on_inconsistent_topic)(void*, zerodds_Topic*);
} zerodds_TopicListener;
```

### §2.5 DataWriterListener (DDS 1.4 §2.2.4.2.5)

See §1.1.

### §2.6 DataReaderListener (DDS 1.4 §2.2.4.2.6)

```c
typedef struct {
    void* user_data;
    void (*on_data_available)(void*, zerodds_DataReader*);
    void (*on_sample_rejected)(void*, zerodds_DataReader*);
    void (*on_liveliness_changed)(void*, zerodds_DataReader*);
    void (*on_requested_deadline_missed)(void*, zerodds_DataReader*);
    void (*on_requested_incompatible_qos)(void*, zerodds_DataReader*);
    void (*on_subscription_matched)(void*, zerodds_DataReader*);
    void (*on_sample_lost)(void*, zerodds_DataReader*);
} zerodds_DataReaderListener;
```

## §3 Status-mask semantics (DDS 1.4 §2.2.4.2.1.4)

`status_mask` is a bitmask over `dds::core::status::StatusKind`. A
callback fires only when the corresponding status bit is set in the mask
**and** the function pointer is non-NULL.

```
StatusMask bit                     | Callback                       | Entity types
-----------------------------------|--------------------------------|------------------
INCONSISTENT_TOPIC          (0x01) | on_inconsistent_topic          | DP, Topic
OFFERED_DEADLINE_MISSED     (0x02) | on_offered_deadline_missed     | Pub, DW
REQUESTED_DEADLINE_MISSED   (0x04) | on_requested_deadline_missed   | Sub, DR
OFFERED_INCOMPATIBLE_QOS    (0x20) | on_offered_incompatible_qos    | Pub, DW
REQUESTED_INCOMPATIBLE_QOS  (0x40) | on_requested_incompatible_qos  | Sub, DR
SAMPLE_LOST                 (0x80) | on_sample_lost                 | Sub, DR
SAMPLE_REJECTED            (0x100) | on_sample_rejected             | Sub, DR
DATA_ON_READERS            (0x200) | on_data_on_readers             | DP, Sub
DATA_AVAILABLE             (0x400) | on_data_available              | Sub, DR
LIVELINESS_LOST            (0x800) | on_liveliness_lost             | Pub, DW
LIVELINESS_CHANGED        (0x1000) | on_liveliness_changed          | Sub, DR
PUBLICATION_MATCHED       (0x2000) | on_publication_matched         | Pub, DW
SUBSCRIPTION_MATCHED      (0x4000) | on_subscription_matched        | Sub, DR
```

`status_mask = 0xFFFFFFFF` activates every non-NULL pointer.

## §4 Threading contract

### §4.1 Caller-driven poll delivery

Callbacks are delivered from `zerodds_poll_listeners()`, which the caller
invokes periodically from its own event loop (Tokio tick, .NET timer,
Python asyncio, JS `setInterval`). They are never delivered synchronously
inside a `set_listener`/`write`/`take` call. This satisfies DDS 1.4
§2.2.4.0 ("listener calls happen in implementation-specific threads") —
here the implementation-specific thread is the caller's poll thread,
which keeps the model lock-free and free of reentrancy hazards.

### §4.2 Re-entrancy

Inside a callback the caller may issue DDS read operations (`take`,
`read`, `get_qos`, status reads). The caller must not free the listener
struct or the entity from within a callback.

### §4.3 Lifetime

`zerodds_*_set_listener(entity, ptr, mask)` stores only a pointer; the
`Zerodds*Listener` struct stays owned by the caller. The caller must keep
the struct alive until it calls `set_listener(entity, NULL, 0)` or the
entity is deleted.

## §5 Aggregator model — caller-driven multi-bind

ZeroDDS replaces the optional DDS bubble-up path (DDS 1.4 §2.2.4.2.0,
"if no listener is attached, propagate to the parent") with **independent
multi-level firing**:

- A listener bound at any level (DataWriter, DataReader, Publisher,
  Subscriber, DomainParticipant, Topic) is evaluated independently each
  poll.
- The Publisher/Subscriber/DomainParticipant listeners aggregate their
  contained entities: the poll walks the parent's child list and fires the
  parent callback for each child status delta.
- There is **no first-match suppression** — if both a DataWriter listener
  and its Publisher listener are bound, both fire for the same event.
- The same `Zerodds*Listener` pointer may be bound to several entities;
  the callback distinguishes the source entity from the entity pointer
  argument and `user_data`.

This gives the caller full control of the aggregation hierarchy without
the first-match race inherent in bubble-up, while still delivering the
parent-level aggregate notifications DDS applications expect.

## §6 Active firing

`zerodds_poll_listeners()` reads the current status counters of every
observed entity, compares them against the per-entity snapshot cached
from the previous poll, and fires the callbacks whose counter advanced
and whose mask bit is set. Counter snapshots are updated once per poll
after all observers have been evaluated, so an event fires for every bound
level in the same poll and exactly once across polls. The function
returns the number of callbacks fired.

### §6.1 Writer statuses

`on_publication_matched`, `on_liveliness_lost`,
`on_offered_deadline_missed`, `on_offered_incompatible_qos` fire on a
delta of the corresponding writer counter, at the DataWriter level and at
the Publisher aggregator.

### §6.2 Reader statuses

`on_subscription_matched`, `on_sample_lost`,
`on_requested_deadline_missed`, `on_requested_incompatible_qos`,
`on_liveliness_changed`, `on_sample_rejected` fire on a delta of the
corresponding reader counter, at the DataReader level and at the
Subscriber aggregator. `on_liveliness_changed` tracks the combined
alive/not-alive change count; `on_sample_rejected` tracks the rejected
total count.

### §6.3 Data availability (set semantics)

`on_data_available` fires on a delta of a monotonic delivered-sample
counter maintained per reader (a non-consuming detector that never false-
fires on the deadline path). `on_data_on_readers` fires with **set
semantics**: once per poll per subscriber that has at least one reader
with a fresh data delta — never once per sample. The DomainParticipant
aggregator fires `on_data_on_readers` once per such subscriber.

### §6.4 Inconsistent topic

ZeroDDS prevents inconsistent topics locally (`create_topic` returns
`PRECONDITION_NOT_MET` for a same-name/different-type clash). The
remaining DDS 1.4 §2.2.4.2.4 case — a remote endpoint discovered with the
same `topic_name` but a different `type_name` — is detected during SEDP
matching: the runtime bumps an inconsistent-topic counter, and the poll
fires `on_inconsistent_topic` on the TopicListener and the
DomainParticipant listener on a counter delta.

## §7 Cross-language mapping

Each binding exposes the listener contract in its idiomatic form; all
forms are implemented.

### §7.1 C++ (DDS-PSM-Cxx 1.0 §7.5.9)

`crates/cpp/include/dds/pub/DataWriterListener.hpp` and
`dds/sub/DataReaderListener.hpp` wrap the C vtable: each method is an
`extern "C"` shim that casts `user_data` back to the C++ listener and
calls the virtual method.

### §7.2 C# (.NET idiom)

`crates/cs/csharp/ZeroDDS/src/Listener.cs` exposes
`IDataWriterListener<T>` plus a bridge that holds
`GCHandle.ToIntPtr(handle)` as `user_data` — NativeAOT-compatible without
reflection. `ZeroDDS.Listener.ListenerPoll.PollAll()` drives
`zerodds_poll_listeners()`.

### §7.3 Java (DDS-Java-PSM 1.0 §8.7)

The pure-Java stack `crates/java-omgdds/java/` registers
`org.omg.dds.*` listeners as Java-heap objects on the `InProcessBus`;
sample arrival invokes the Java method directly. For the multi-process
case `org.zerodds.bridge.GrpcBridgeClient` drives a DDS runtime in a
separate process over gRPC. Both paths deliver listener callbacks without
a C-FFI vtable detour.

### §7.4 Python (PyO3 idiom)

`crates/py/src/ffi.rs` exposes a caller-driven polling API
(`wait_for_data`, `wait_for_matched_subscription`,
`wait_for_matched_publication`). This is the idiomatic listener-equivalent
under the GIL — the caller waits on the status it cares about rather than
receiving cross-thread callbacks.

### §7.5 TypeScript (Node + WASM)

`crates/ts-node/src/dds.ts` exposes `DataReader.waitForMatched(min,
timeoutMs)` and the writer counterpart as a caller-driven polling API,
matching the single-threaded Node event loop.

## §8 Test obligations

1. Identity round-trip: `set_listener(entity, l, FULL_MASK)` then
   `get_listener(entity)` returns the same pointer.
2. `set_listener(entity, NULL, 0)` clears.
3. The poll fires the status callbacks on a counter delta and fires
   nothing when there is no delta.
4. The aggregator levels fire for child entity deltas
   (Publisher/Subscriber/DomainParticipant) including `on_data_on_readers`
   set semantics.
5. `on_inconsistent_topic` fires on the Topic and DomainParticipant
   listeners on a counter delta.

## §9 Memory ownership

| Operation                                     | Owner                              |
|-----------------------------------------------|------------------------------------|
| `Zerodds*Listener` struct allocated by caller | Caller                             |
| `set_listener(entity, ptr, mask)`             | Registry holds a weak raw pointer  |
| `set_listener(entity, NULL, 0)`               | Registry clears the entry          |
| Caller frees the listener struct              | Must call `set_listener(NULL)` first |

## §10 Stability

Vendor spec, semver:

- Breaking changes require a v2.0 major bump.
- v1.x additions are backwards-compatible (new fields appended at the
  struct end; existing callers stay compatible).

## §11 Spec-conformance notes

All DDS 1.4 §2.2.4 listener methods are exposed 1:1 as C-FFI function
pointers and actively fired. The `void* user_data` convention is a vendor
detail (the DDS spec defines listeners as classes without a separate
`user_data` field) but is required for the C-FFI mapping.
