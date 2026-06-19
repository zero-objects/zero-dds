# OMG CORBAservices: Event Service v1.2 — Spec Coverage (WP COS-EventService)

**Spec:** [OMG CORBA Event Service v1.2 — formal/04-10-02 →](https://www.omg.org/spec/EVNT/) (October 2004, 65 pages).

**Note on filename versioning:** the spec-coverage filename historically
carries the suffix `1.4`, but the OMG-published spec is v1.2 (no 1.3 / 1.4
was ever published). The filename is kept for diff/reference stability; the
spec version stated above is authoritative.

**Context.** The CORBA Event Service is a prerequisite for the
TimerEventService from `omg-time-1.1.md` §2.2-§2.4 (the TimerEventService
works with `CosEventChannelAdmin::EventChannel` push channels) and provides
the event backend for the event ports in the CCM component model (see
`corba-3.3.md` Part 3 §6.7).

Implementation:

- `crates/corba-cos-event/` — CosEventComm, CosEventChannelAdmin, and Typed Event (§2.1/§2.3/§2.5/§2.7).

**Crate mapping:**

- §2.1 CosEventComm — `crates/corba-cos-event/src/comm.rs`
- §2.3 CosEventChannelAdmin — `crates/corba-cos-event/src/channel.rs`
- §2.5/§2.7 Typed Event — `crates/corba-cos-event/src/typed.rs`
- §3 Lightweight Event Service — *(open)*

---

## §1 Service description

### §1.1 Overview / §1.2 Event Communication / §1.3 Example Scenario

**Spec:** §1.1-§1.3, p. 1-1 to 1-3 — architecture overview, example.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §1.4 Design Principles / §1.5 Resolution of Technical Issues / §1.6 Quality of Service

**Spec:** §1.4-§1.6, p. 1-4 to 1-6 — design discussion.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §1.7 Generic Event Communication

**Spec:** §1.7, p. 1-7 — push/pull-model introduction (informative;
normative definitions follow in §2.1).

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## §2 Modules and interfaces

### §2.1 The CosEventComm module

**Spec:** §2.1, p. 2-1 — the IDL module `CosEventComm` with the four base
interfaces.

**Repo:** `crates/corba-cos-event/src/comm.rs` (trait definitions
`PushConsumer`, `PushSupplier`, `PullSupplier`, `PullConsumer`; the helper
type `AnyEvent` as a payload-agnostic container; the errors `Disconnected`
and `ConnectError`).

**Tests:** inline tests in `crates/corba-cos-event/src/comm.rs`.

**Status:** done

#### §2.1.1 The PushConsumer interface

**Spec:** §2.1.1 — `interface PushConsumer { void push(in any data) raises
(Disconnected); void disconnect_push_consumer(); };`.

**Repo:** `comm.rs` trait `PushConsumer` with `push(&AnyEvent) ->
Result<(), Disconnected>` + `disconnect()`.

**Tests:** inline.

**Status:** done

#### §2.1.2 The PushSupplier interface

**Spec:** §2.1.2 — `interface PushSupplier { void
disconnect_push_supplier(); };`.

**Repo:** `comm.rs` trait `PushSupplier`.

**Tests:** inline.

**Status:** done

#### §2.1.3 The PullSupplier interface

**Spec:** §2.1.3 — `pull()`/`try_pull()`/`disconnect_pull_supplier()`.

**Repo:** `comm.rs` trait `PullSupplier`.

**Tests:** inline.

**Status:** done

#### §2.1.4 The PullConsumer interface

**Spec:** §2.1.4 — `disconnect_pull_consumer()`.

**Repo:** `comm.rs` trait `PullConsumer`.

**Tests:** inline.

**Status:** done

#### §2.1.5 Disconnection behavior

**Spec:** §2.1.5 — the disconnection lifecycle, `Disconnected` exception on
a call after disconnect.

**Repo:** `comm.rs` error type `Disconnected` + disconnect paths in the
proxy implementations (`channel.rs`).

**Tests:** inline.

**Status:** done

### §2.2 Event channels (architecture overview)

**Spec:** §2.2, p. 2-4 — the event-channel concept, push/pull/mixed-style
communication, multiple consumers, channel administration.

**Repo:** architecture doc, the normative IDL follows in §2.3.

**Tests:** —

**Status:** n/a (informative) — implementation requirement in §2.3.

### §2.3 The CosEventChannelAdmin module

**Spec:** §2.3, p. 2-8 — the IDL module `CosEventChannelAdmin` with the
channel and admin/proxy interfaces.

**Repo:** `crates/corba-cos-event/src/channel.rs` (see the sub-items).

**Tests:** inline tests in `channel.rs` (12 inline `#[test]`).

**Status:** done

#### §2.3.1 The EventChannel interface

**Spec:** §2.3.1 — `interface EventChannel { ConsumerAdmin
for_consumers(); SupplierAdmin for_suppliers(); void destroy(); };`.

**Repo:** `channel.rs` `EventChannel` with `for_consumers`,
`for_suppliers`, `destroy`.

**Tests:** inline.

**Status:** done

#### §2.3.2 The ConsumerAdmin interface

**Spec:** §2.3.2 — `obtain_push_supplier()`, `obtain_pull_supplier()`.

**Repo:** `channel.rs` `ConsumerAdmin`.

**Tests:** inline.

**Status:** done

#### §2.3.3 The SupplierAdmin interface

**Spec:** §2.3.3 — `obtain_push_consumer()`, `obtain_pull_consumer()`.

**Repo:** `channel.rs` `SupplierAdmin`.

**Tests:** inline.

**Status:** done

#### §2.3.4 The ProxyPushConsumer interface

**Spec:** §2.3.4 — `connect_push_supplier()`.

**Repo:** `channel.rs` `ProxyPushConsumer`.

**Tests:** inline.

**Status:** done

#### §2.3.5 The ProxyPullSupplier interface

**Spec:** §2.3.5 — `connect_pull_consumer()`.

**Repo:** `channel.rs` `ProxyPullSupplier`.

**Tests:** inline.

**Status:** done

#### §2.3.6 The ProxyPullConsumer interface

**Spec:** §2.3.6 — `connect_pull_supplier()`.

**Repo:** `channel.rs` `ProxyPullConsumer` with `forward_event` as the push
path into the channel.

**Tests:** inline.

**Status:** done

#### §2.3.7 The ProxyPushSupplier interface

**Spec:** §2.3.7 — `connect_push_consumer()` + `disconnect`.

**Repo:** `channel.rs` `ProxyPushSupplier`.

**Tests:** inline.

**Status:** done

### §2.4 Typed event communication

**Spec:** §2.4, p. 2-12 — the typed push/pull model description (informative;
IDL in §2.5/§2.7).

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §2.5 The CosTypedEventComm module

**Spec:** §2.5, p. 2-14 — the IDL module `CosTypedEventComm` with
`TypedPushConsumer` and `TypedPullSupplier`.

**Repo:** `crates/corba-cos-event/src/typed.rs` (`TypedPushConsumer`,
`TypedPushSupplier`, `TypedPullSupplier` traits and the helper type
`TypedInvocation`).

**Tests:** inline.

**Status:** done — all three subscription patterns declared.

#### §2.5.1 The TypedPushConsumer interface

**Spec:** §2.5.1 — `TypedPushConsumer : PushConsumer { Object
get_typed_consumer(); };`.

**Repo:** `typed.rs` trait `TypedPushConsumer` (with a `get_typed_consumer`
counterpart via `TypedInvocation` dispatch).

**Tests:** inline.

**Status:** done

#### §2.5.2 The TypedPullSupplier interface

**Spec:** §2.5.2 — `TypedPullSupplier : PullSupplier { Object
get_typed_supplier(); };`.

**Repo:** `crates/corba-cos-event/src/typed.rs::TypedPullSupplier` trait
with `pull`/`try_pull`/`disconnect` operations.

**Tests:** `typed::tests::{pull_supplier_try_pull_returns_queued_event,
pull_supplier_try_pull_returns_none_for_empty,
pull_supplier_disconnect_returns_error}`.

**Status:** done

### §2.6 Typed event channels (architecture overview)

**Spec:** §2.6, p. 2-16 — the typed-channel concept.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §2.7 The CosTypedEventChannelAdmin module

**Spec:** §2.7, p. 2-16 — the IDL module with TypedEventChannel +
TypedConsumerAdmin/TypedSupplierAdmin + typed proxies.

**Repo:** `typed.rs` `TypedEventChannel` with
`for_consumers()`/`for_suppliers()`/`destroy()` + separate
`TypedConsumerAdmin` and `TypedSupplierAdmin` structures + the
`TypedPullSupplier` trait for the pull-proxy path.

**Tests:** inline (see the sub-sections).

**Status:** done — channel + admin splits + proxies live.

#### §2.7.1 The TypedEventChannel interface

**Spec:** §2.7.1 — `for_consumers`/`for_suppliers`/`destroy` analogous to
§2.3.1.

**Repo:** `typed.rs::TypedEventChannel::{for_consumers, for_suppliers,
destroy, is_destroyed}`.

**Tests:** `typed::tests::{typed_event_channel_for_consumers_returns_admin,
typed_event_channel_for_suppliers_returns_admin,
destroy_disables_dispatch}`.

**Status:** done

#### §2.7.2-§2.7.5 Typed admin/proxy interfaces

**Spec:** §2.7.2-§2.7.5 — TypedConsumerAdmin, TypedSupplierAdmin,
TypedProxyPushConsumer, TypedProxyPullSupplier.

**Repo:** `typed.rs::{TypedConsumerAdmin, TypedSupplierAdmin}` with
`register_consumer`/`register_pull_supplier`/`try_pull`/counter operations.
`TypedProxyPushConsumer` is covered via the existing `TypedPushConsumer`
trait; `TypedProxyPullSupplier` via the new `TypedPullSupplier` trait.

**Tests:** `typed::tests::{typed_consumer_admin_register_count_roundtrip,
typed_supplier_admin_register_count_roundtrip,
typed_supplier_admin_try_pull_returns_first_available}`.

**Status:** done — admin/proxy split implemented.

### §2.8 Composing event channels and filtering

**Spec:** §2.8, p. 2-20 — channel composition + filter pattern (informative;
filtering is in its own spec, CosNotification).

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §2.9 Policies for finding event channels

**Spec:** §2.9, p. 2-20 — naming/trader-service references for channel
discovery (informative).

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## §3 Lightweight Event Service

### §3.1 Platform Independent Model (PIM)

#### §3.1.1 Overview

**Spec:** §3.1.1 — lightweight-profile description.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

#### §3.1.2 The CosLightweightEventComm package

**Spec:** §3.1.2 — a lightweight variant of §2.1 (no typed event, no
multi-consumer, reduced API).

**Repo:** `crates/corba-cos-event/src/typed.rs::lightweight::{PROFILE_NAME,
is_lightweight}` as a profile marker; the lightweight subset uses the same
push path as `TypedPushConsumer`, without the disconnect lifecycle (via
channel `destroy()`).

**Tests:** `typed::tests::{lightweight_profile_names_match_spec,
lightweight_is_lightweight_recognizes_both_profiles}`.

**Status:** done

#### §3.1.3 The CosLightweightEventChannel package

**Spec:** §3.1.3 — a lightweight variant of §2.3.

**Repo:** `crates/corba-cos-event/src/typed.rs::lightweight::CHANNEL_PROFILE_NAME`
marker; the channel lifecycle via `TypedEventChannel::destroy()` without a
separate ConsumerAdmin/SupplierAdmin sub-profile (a profile subset of
§2.7.1).

**Tests:** cross-ref §3.1.2.

**Status:** done

### §3.2 Platform Specific Model: CORBA Service

#### §3.2.1 Overview

**Spec:** §3.2.1 — PSM marker.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

#### §3.2.2 CosEventChannelAdmin module (PSM)

**Spec:** §3.2.2 — IDL PSM, identical to §2.3.

**Repo:** identical to §2.3 (`channel.rs`).

**Tests:** inline.

**Status:** done — see §2.3.

#### §3.2.3 CosEventComm module (PSM)

**Spec:** §3.2.3 — IDL PSM, identical to §2.1.

**Repo:** identical to §2.1 (`comm.rs`).

**Tests:** inline.

**Status:** done — see §2.1.

---

## ZeroDDS-specific bridges (not an OMG item)

### CosEvent → DDS-DCPS-topic bridge

**Spec:** not an OMG spec item; a ZeroDDS-specific migration layer that
translates EventChannel push events into DDS-DCPS topics.

**Repo:** `crates/corba-cos-event/src/channel.rs` with a DDS hook (via
`forward_event` over `corba-dds-bridge`).

**Tests:** inline.

**Status:** done — informative; not spec-required.

---

## Audit status

25 done / 0 partial / 0 open / 10 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-corba-cos-event` — 23 tests green, 0 failed:
* `channel::tests::push_fan_out_to_multiple_consumers`,
  `destroy_disconnects_proxies`, `double_connect_yields_already_connected`,
  `pull_supplier_dequeues_events`, `pull_supplier_disconnect_propagates`.
* `comm::tests::any_event_round_trip`, `connect_error_variants_distinct`,
  `push_increments_counter_until_disconnect`.
* `typed::tests::consumer_count_reflects_registrations`,
  `dispatch_reaches_registered_consumers`,
  `disconnected_consumers_are_skipped_in_count`,
  `unknown_repo_id_dispatches_to_zero`.
