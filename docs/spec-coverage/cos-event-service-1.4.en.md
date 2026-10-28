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
- §3 Lightweight Event Service — `crates/corba-cos-event/src/typed.rs` (`lightweight`)
- CosEvent → DDS topic bridge (not an OMG item) — `crates/corba-cos-event/src/dds_bridge.rs`

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

**Tests:** `crates/corba-cos-event/src/comm.rs::tests`:
`push_increments_counter_until_disconnect`,
`push_supplier_disconnect_terminates_the_connection`,
`pull_supplier_try_pull_returns_the_has_event_flag`,
`pull_consumer_disconnect_terminates_the_connection`,
`any_event_round_trip`, `connect_error_variants_distinct`.

**Status:** done

#### §2.1.1 The PushConsumer interface

**Spec:** §2.1.1 — `interface PushConsumer { void push(in any data) raises
(Disconnected); void disconnect_push_consumer(); };`.

**Repo:** `crates/corba-cos-event/src/comm.rs` trait `PushConsumer` with
`push(&self, event: AnyEvent) -> Result<(), Disconnected>` +
`disconnect_push_consumer(&self)`.

**Tests:** `crates/corba-cos-event/src/comm.rs::tests::push_increments_counter_until_disconnect`
(`push` counts up to the disconnect, `Disconnected` afterwards);
`crates/corba-cos-event/src/channel.rs::tests::proxy_push_consumer_push_before_connect_raises_disconnected`
(`push` before `connect` yields `Disconnected`).

**Status:** done

#### §2.1.2 The PushSupplier interface

**Spec:** §2.1.2 — `interface PushSupplier { void
disconnect_push_supplier(); };`.

**Repo:** `crates/corba-cos-event/src/comm.rs` trait `PushSupplier`; the channel-side implementation
`crates/corba-cos-event/src/channel.rs::ProxyPushSupplier` (`impl PushSupplier for ProxyPushSupplier`)
— the disconnect unregisters the consumer from the channel and calls its
`disconnect_push_consumer`.

**Tests:** `crates/corba-cos-event/src/comm.rs::tests::push_supplier_disconnect_terminates_the_connection`;
`crates/corba-cos-event/src/channel.rs::tests::proxy_push_supplier_disconnect_stops_event_delivery`
(no further event reaches the consumer after the disconnect).

**Status:** done

#### §2.1.3 The PullSupplier interface

**Spec:** §2.1.3 — `pull()`/`try_pull()`/`disconnect_pull_supplier()`;
`pull` blocks until an event is available, `try_pull` additionally returns
the `has_event` flag.

**Repo:** `crates/corba-cos-event/src/comm.rs` trait `PullSupplier` with
`pull() -> Result<AnyEvent, Disconnected>`,
`try_pull() -> Result<(AnyEvent, bool), Disconnected>`,
`disconnect_pull_supplier()`.

**Tests:** `crates/corba-cos-event/src/comm.rs::tests`:
`pull_supplier_try_pull_returns_the_has_event_flag` (flag `true` with the
event, `false` with an empty `any`),
`pull_supplier_pull_returns_the_queued_event`,
`pull_supplier_disconnect_raises_disconnected_on_both_operations`.

**Status:** done

#### §2.1.4 The PullConsumer interface

**Spec:** §2.1.4 — `disconnect_pull_consumer()`.

**Repo:** `crates/corba-cos-event/src/comm.rs` trait `PullConsumer`; the channel-side implementation
`crates/corba-cos-event/src/channel.rs::ProxyPullConsumer`
(`impl PullConsumer for ProxyPullConsumer`).

**Tests:** `crates/corba-cos-event/src/comm.rs::tests::pull_consumer_disconnect_terminates_the_connection`;
`crates/corba-cos-event/src/channel.rs::tests::proxy_pull_consumer_connect_and_disconnect_lifecycle`.

**Status:** done

#### §2.1.5 Disconnection behavior

**Spec:** §2.1.5 — the disconnection lifecycle, `Disconnected` exception on
a call after disconnect.

**Repo:** `crates/corba-cos-event/src/comm.rs` error type `Disconnected` + disconnect paths in the
proxy implementations (`crates/corba-cos-event/src/channel.rs`).

**Tests:** `crates/corba-cos-event/src/comm.rs::tests::disconnection_is_terminal_for_every_operation`
(the disconnect is terminal and idempotent; every operation reports
`Disconnected` afterwards);
`crates/corba-cos-event/src/channel.rs::tests::push_unregisters_a_consumer_that_reports_disconnected`
(a consumer reporting `Disconnected` is unregistered instead of being
skipped silently);
`crates/corba-cos-event/src/channel.rs::tests::destroy_wakes_blocked_pull_with_disconnected`.

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

**Tests:** `crates/corba-cos-event/src/channel.rs::tests`:
`event_channel_admins_share_one_channel_state`,
`consumer_admin_returns_a_fresh_proxy_per_call`,
`supplier_admin_returns_a_fresh_proxy_per_call`,
`push_fan_out_to_multiple_consumers`,
`proxy_pull_consumer_forwards_pulled_events_into_the_channel`.

**Status:** done

#### §2.3.1 The EventChannel interface

**Spec:** §2.3.1 — `interface EventChannel { ConsumerAdmin
for_consumers(); SupplierAdmin for_suppliers(); void destroy(); };`;
`destroy` releases the channel and disconnects the attached endpoints.

**Repo:** `crates/corba-cos-event/src/channel.rs` `EventChannel` with `for_consumers`,
`for_suppliers`, `destroy`, `is_destroyed`.

**Tests:** `crates/corba-cos-event/src/channel.rs::tests`:
`event_channel_admins_share_one_channel_state` (both admin views address
the same channel state),
`destroy_disconnects_registered_consumers` (`destroy` calls
`disconnect_push_consumer` on every registered consumer),
`destroy_disconnects_proxies`,
`destroy_wakes_blocked_pull_with_disconnected`.

**Status:** done

#### §2.3.2 The ConsumerAdmin interface

**Spec:** §2.3.2 — `obtain_push_supplier()`, `obtain_pull_supplier()`; each
call returns a proxy of its own.

**Repo:** `crates/corba-cos-event/src/channel.rs` `ConsumerAdmin`.

**Tests:** `crates/corba-cos-event/src/channel.rs::tests::consumer_admin_returns_a_fresh_proxy_per_call`
(two calls return distinct proxies; the second one is unconnected and
accepts a consumer of its own).

**Status:** done

#### §2.3.3 The SupplierAdmin interface

**Spec:** §2.3.3 — `obtain_push_consumer()`, `obtain_pull_consumer()`; each
call returns a proxy of its own.

**Repo:** `crates/corba-cos-event/src/channel.rs` `SupplierAdmin`.

**Tests:** `crates/corba-cos-event/src/channel.rs::tests::supplier_admin_returns_a_fresh_proxy_per_call`.

**Status:** done

#### §2.3.4 The ProxyPushConsumer interface

**Spec:** §2.3.4 — `connect_push_supplier()` raises `AlreadyConnected`; a
`push` before the connect is `Disconnected`.

**Repo:** `crates/corba-cos-event/src/channel.rs` `ProxyPushConsumer` (`connect_push_supplier`,
`impl PushConsumer`).

**Tests:** `crates/corba-cos-event/src/channel.rs::tests`:
`proxy_push_consumer_push_before_connect_raises_disconnected`,
`double_connect_yields_already_connected`,
`push_fan_out_to_multiple_consumers`.

**Status:** done

#### §2.3.5 The ProxyPullSupplier interface

**Spec:** §2.3.5 — `connect_pull_consumer()` raises `AlreadyConnected`;
`pull` blocks until an event is available, `try_pull` returns the
`has_event` flag.

**Repo:** `crates/corba-cos-event/src/channel.rs` `ProxyPullSupplier` (`connect_pull_consumer`,
`impl PullSupplier`; the blocking `pull` waits on the `Condvar`
`ChannelInner::ready` rather than in a poll loop).

**Tests:** `crates/corba-cos-event/src/channel.rs::tests`:
`proxy_pull_supplier_try_pull_reports_has_event_flag`,
`proxy_pull_supplier_pull_blocks_until_an_event_arrives` (the call returns
only after a supplier has pushed),
`proxy_pull_supplier_double_connect_yields_already_connected`,
`pull_supplier_dequeues_events`, `pull_supplier_disconnect_propagates`.

**Status:** done

#### §2.3.6 The ProxyPullConsumer interface

**Spec:** §2.3.6 — `connect_pull_supplier()` raises `AlreadyConnected`;
inherited `disconnect_pull_consumer()`.

**Repo:** `crates/corba-cos-event/src/channel.rs` `ProxyPullConsumer` with `connect_pull_supplier`,
`forward_event` as the push path into the channel, and `impl PullConsumer`
for the disconnect.

**Tests:** `crates/corba-cos-event/src/channel.rs::tests`:
`proxy_pull_consumer_forwards_pulled_events_into_the_channel` (the events
pulled from the supplier show up on the consumer side of the channel),
`proxy_pull_consumer_connect_and_disconnect_lifecycle`.

**Status:** done

#### §2.3.7 The ProxyPushSupplier interface

**Spec:** §2.3.7 — `connect_push_consumer()` raises `AlreadyConnected`;
`disconnect_push_supplier()` terminates the connection.

**Repo:** `crates/corba-cos-event/src/channel.rs` `ProxyPushSupplier` with `connect_push_consumer`,
`is_connected`, and `impl PushSupplier` (`disconnect_push_supplier`).

**Tests:** `crates/corba-cos-event/src/channel.rs::tests`:
`proxy_push_supplier_double_connect_yields_already_connected`,
`proxy_push_supplier_disconnect_stops_event_delivery`.

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

**Tests:** `crates/corba-cos-event/src/typed.rs::tests`:
`typed_push_consumer_invoke_carries_operation_and_args`,
`typed_push_supplier_disconnect_terminates_the_connection`,
`pull_supplier_try_pull_returns_queued_event`.

**Status:** done — all three subscription patterns declared.

#### §2.5.1 The TypedPushConsumer interface

**Spec:** §2.5.1 — `TypedPushConsumer : PushConsumer { Object
get_typed_consumer(); };`; in typed mode the channel calls the IDL
operation instead of `push(any)`.

**Repo:** `typed.rs` trait `TypedPushConsumer` (with a `get_typed_consumer`
counterpart via `TypedInvocation` dispatch).

**Tests:** `crates/corba-cos-event/src/typed.rs::tests`:
`typed_push_consumer_invoke_carries_operation_and_args` (operation name and
encoded arguments reach the consumer, the reply travels back),
`typed_push_consumer_disconnect_raises_disconnected`,
`dispatch_reaches_registered_consumers`.

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

**Tests:** `crates/corba-cos-event/src/typed.rs::tests`:
`typed_event_channel_for_consumers_returns_admin`,
`typed_event_channel_for_suppliers_returns_admin`,
`typed_consumer_admin_register_count_roundtrip`,
`typed_supplier_admin_try_pull_returns_first_available`,
`destroy_disables_dispatch`.

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

**Spec:** §3.1.3 — a lightweight variant of §2.3: the channel lifecycle is
the only teardown, the profile has no endpoint disconnect operation.

**Repo:** `crates/corba-cos-event/src/typed.rs::lightweight::CHANNEL_PROFILE_NAME`
marker; the channel lifecycle via `TypedEventChannel::destroy()` without a
separate ConsumerAdmin/SupplierAdmin sub-profile (a profile subset of
§2.7.1).

**Tests:** `crates/corba-cos-event/src/typed.rs::tests::lightweight_channel_destroy_is_the_only_teardown`
(a consumer without a disconnect of its own receives no dispatches after
`destroy()`);
`crates/corba-cos-event/src/typed.rs::tests::lightweight_profile_names_match_spec`.

**Status:** done

### §3.2 Platform Specific Model: CORBA Service

#### §3.2.1 Overview

**Spec:** §3.2.1 — PSM marker.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

#### §3.2.2 CosEventChannelAdmin module (PSM)

**Spec:** §3.2.2 — IDL PSM, identical to §2.3.

**Repo:** identical to §2.3 (`crates/corba-cos-event/src/channel.rs`).

**Tests:** identical to §2.3 —
`crates/corba-cos-event/src/channel.rs::tests`:
`event_channel_admins_share_one_channel_state`,
`consumer_admin_returns_a_fresh_proxy_per_call`,
`supplier_admin_returns_a_fresh_proxy_per_call`,
`proxy_push_consumer_push_before_connect_raises_disconnected`,
`proxy_pull_supplier_try_pull_reports_has_event_flag`,
`proxy_pull_consumer_forwards_pulled_events_into_the_channel`,
`proxy_push_supplier_disconnect_stops_event_delivery`.

**Status:** done — see §2.3.

#### §3.2.3 CosEventComm module (PSM)

**Spec:** §3.2.3 — IDL PSM, identical to §2.1.

**Repo:** identical to §2.1 (`crates/corba-cos-event/src/comm.rs`).

**Tests:** identical to §2.1 —
`crates/corba-cos-event/src/comm.rs::tests`:
`push_increments_counter_until_disconnect`,
`push_supplier_disconnect_terminates_the_connection`,
`pull_supplier_try_pull_returns_the_has_event_flag`,
`pull_supplier_pull_returns_the_queued_event`,
`pull_consumer_disconnect_terminates_the_connection`,
`disconnection_is_terminal_for_every_operation`.

**Status:** done — see §2.1.

---

## ZeroDDS-specific bridges (not an OMG item)

### CosEvent → DDS-DCPS-topic bridge

**Spec:** not an OMG spec item; a ZeroDDS-specific migration layer that
translates EventChannel push events into DDS-DCPS topics.

**Repo:** `crates/corba-cos-event/src/dds_bridge.rs` (`TopicBridge` as a
`PushConsumer` on the EventChannel; `TopicMap` as the repository-ID ⇒ topic
table with `derive_topic_name` as the fallback derivation for types that are
not listed; `TopicSink` as the handover point to the caller layer's
DataWriter, which keeps the crate free of a DCPS dependency). An
unresolvable repository ID and a refusing sink both raise an error and count
in `TopicBridge::rejected` — never a silent drop.

**Tests:** `crates/corba-cos-event/src/dds_bridge.rs::tests`:
`derives_topic_name_from_repository_id`,
`explicit_mapping_wins_over_derivation`,
`push_writes_the_payload_onto_the_resolved_topic`,
`unresolvable_repository_id_is_reported_not_dropped`,
`sink_failure_is_reported_not_swallowed`,
`disconnect_stops_forwarding`,
`event_channel_fan_out_reaches_the_dds_topic` (a push into the EventChannel
reaches the DDS topic write).

**Status:** done — informative; not spec-required.

---

## Audit status

25 done / 0 partial / 0 open / 10 n/a (informative) / 0 n/a (rejected).

Open items: none. Decision records: none.

Test run: `cargo test -p zerodds-corba-cos-event` — 54 lib tests + 1 doc
test green, 0 failed.
