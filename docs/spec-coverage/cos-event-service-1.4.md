# OMG CORBAservices: Event Service v1.2 — Spec-Coverage (WP COS-EventService)

**Spec:** [OMG CORBA Event Service v1.2 — formal/04-10-02 →](https://www.omg.org/spec/EVNT/) (Oktober 2004, 65 Seiten).

**Hinweis Filename-Versionierung:** Der Spec-Coverage-Filename trägt
historisch das Suffix `1.4`, die OMG-veröffentlichte Spec ist
v1.2 (es gibt keine 1.3 / 1.4 publiziert). Filename bleibt aus
Diff-/Referenz-Stabilität erhalten; Spec-Versionsangabe oben ist
maßgeblich.

**Kontext.** Der CORBA Event Service ist Voraussetzung für den
TimerEventService aus `omg-time-1.1.md` §2.2-§2.4 (TimerEventService
arbeitet mit `CosEventChannelAdmin::EventChannel`-Push-Channels) und
liefert das Event-Backend für die Event-Ports im CCM-Komponentenmodell
(siehe `corba-3.3.md` Part 3 §6.7).

Implementation:

- `crates/corba-cos-event/` — CosEventComm, CosEventChannelAdmin und Typed-Event (§2.1/§2.3/§2.5/§2.7).

**Crate-Mapping:**

- §2.1 CosEventComm — `crates/corba-cos-event/src/comm.rs`
- §2.3 CosEventChannelAdmin — `crates/corba-cos-event/src/channel.rs`
- §2.5/§2.7 Typed Event — `crates/corba-cos-event/src/typed.rs`
- §3 Lightweight Event Service — *(open)*

---

## §1 Service Description

### §1.1 Overview / §1.2 Event Communication / §1.3 Example Scenario

**Spec:** §1.1-§1.3, S. 1-1 bis 1-3 — Architektur-Überblick, Beispiel.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §1.4 Design Principles / §1.5 Resolution of Technical Issues / §1.6 Quality of Service

**Spec:** §1.4-§1.6, S. 1-4 bis 1-6 — Design-Diskussion.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §1.7 Generic Event Communication

**Spec:** §1.7, S. 1-7 — Push-/Pull-Modell-Einführung (informativ;
normative Definitionen folgen in §2.1).

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## §2 Modules and Interfaces

### §2.1 The CosEventComm Module

**Spec:** §2.1, S. 2-1 — IDL-Modul `CosEventComm` mit den vier Basis-
Interfaces.

**Repo:** `crates/corba-cos-event/src/comm.rs` (Trait-Definitionen
`PushConsumer`, `PushSupplier`, `PullSupplier`, `PullConsumer`;
Helper-Typ `AnyEvent` als payload-agnostischer Container; Error
`Disconnected` und `ConnectError`).

**Tests:** Inline-Tests in `crates/corba-cos-event/src/comm.rs`.

**Status:** done

#### §2.1.1 The PushConsumer Interface

**Spec:** §2.1.1 — `interface PushConsumer { void push(in any data)
raises (Disconnected); void disconnect_push_consumer(); };`.

**Repo:** `comm.rs` Trait `PushConsumer` mit `push(&AnyEvent) ->
Result<(), Disconnected>` + `disconnect()`.

**Tests:** Inline.

**Status:** done

#### §2.1.2 The PushSupplier Interface

**Spec:** §2.1.2 — `interface PushSupplier { void
disconnect_push_supplier(); };`.

**Repo:** `comm.rs` Trait `PushSupplier`.

**Tests:** Inline.

**Status:** done

#### §2.1.3 The PullSupplier Interface

**Spec:** §2.1.3 — `pull()`/`try_pull()`/`disconnect_pull_supplier()`.

**Repo:** `comm.rs` Trait `PullSupplier`.

**Tests:** Inline.

**Status:** done

#### §2.1.4 The PullConsumer Interface

**Spec:** §2.1.4 — `disconnect_pull_consumer()`.

**Repo:** `comm.rs` Trait `PullConsumer`.

**Tests:** Inline.

**Status:** done

#### §2.1.5 Disconnection Behavior

**Spec:** §2.1.5 — Disconnection-Lifecycle, `Disconnected`-Exception
beim Aufruf nach disconnect.

**Repo:** `comm.rs` Error-Typ `Disconnected` + Disconnect-Pfade in den
Proxy-Implementations (`channel.rs`).

**Tests:** Inline.

**Status:** done

### §2.2 Event Channels (Architektur-Überblick)

**Spec:** §2.2, S. 2-4 — Event-Channel-Begriff, Push-/Pull-/Mixed-
Style-Communication, Multiple Consumers, Channel-Administration.

**Repo:** Architektur-Doku, normative IDL folgt in §2.3.

**Tests:** —

**Status:** n/a (informative) — Implementations-Pflicht in §2.3.

### §2.3 The CosEventChannelAdmin Module

**Spec:** §2.3, S. 2-8 — IDL-Modul `CosEventChannelAdmin` mit Channel-
und Admin-/Proxy-Interfaces.

**Repo:** `crates/corba-cos-event/src/channel.rs` (siehe
Sub-Items).

**Tests:** Inline-Tests in `channel.rs` (12 inline `#[test]`).

**Status:** done

#### §2.3.1 The EventChannel Interface

**Spec:** §2.3.1 — `interface EventChannel { ConsumerAdmin
for_consumers(); SupplierAdmin for_suppliers(); void destroy(); };`.

**Repo:** `channel.rs` `EventChannel` mit `for_consumers`,
`for_suppliers`, `destroy`.

**Tests:** Inline.

**Status:** done

#### §2.3.2 The ConsumerAdmin Interface

**Spec:** §2.3.2 — `obtain_push_supplier()`, `obtain_pull_supplier()`.

**Repo:** `channel.rs` `ConsumerAdmin`.

**Tests:** Inline.

**Status:** done

#### §2.3.3 The SupplierAdmin Interface

**Spec:** §2.3.3 — `obtain_push_consumer()`, `obtain_pull_consumer()`.

**Repo:** `channel.rs` `SupplierAdmin`.

**Tests:** Inline.

**Status:** done

#### §2.3.4 The ProxyPushConsumer Interface

**Spec:** §2.3.4 — `connect_push_supplier()`.

**Repo:** `channel.rs` `ProxyPushConsumer`.

**Tests:** Inline.

**Status:** done

#### §2.3.5 The ProxyPullSupplier Interface

**Spec:** §2.3.5 — `connect_pull_consumer()`.

**Repo:** `channel.rs` `ProxyPullSupplier`.

**Tests:** Inline.

**Status:** done

#### §2.3.6 The ProxyPullConsumer Interface

**Spec:** §2.3.6 — `connect_pull_supplier()`.

**Repo:** `channel.rs` `ProxyPullConsumer` mit `forward_event` als
Push-Pfad in den Channel.

**Tests:** Inline.

**Status:** done

#### §2.3.7 The ProxyPushSupplier Interface

**Spec:** §2.3.7 — `connect_push_consumer()` + `disconnect`.

**Repo:** `channel.rs` `ProxyPushSupplier`.

**Tests:** Inline.

**Status:** done

### §2.4 Typed Event Communication

**Spec:** §2.4, S. 2-12 — Typed-Push/Pull-Modell-Beschreibung
(informativ; IDL in §2.5/§2.7).

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §2.5 The CosTypedEventComm Module

**Spec:** §2.5, S. 2-14 — IDL-Modul `CosTypedEventComm` mit
`TypedPushConsumer` und `TypedPullSupplier`.

**Repo:** `crates/corba-cos-event/src/typed.rs` (`TypedPushConsumer`,
`TypedPushSupplier`, `TypedPullSupplier`-Traits und Helper-Typ
`TypedInvocation`).

**Tests:** Inline.

**Status:** done — alle drei Subscription-Patterns ausgewiesen.

#### §2.5.1 The TypedPushConsumer Interface

**Spec:** §2.5.1 — `TypedPushConsumer : PushConsumer { Object
get_typed_consumer(); };`.

**Repo:** `typed.rs` Trait `TypedPushConsumer` (mit
`get_typed_consumer`-Pendant via `TypedInvocation`-Dispatch).

**Tests:** Inline.

**Status:** done

#### §2.5.2 The TypedPullSupplier Interface

**Spec:** §2.5.2 — `TypedPullSupplier : PullSupplier { Object
get_typed_supplier(); };`.

**Repo:** `crates/corba-cos-event/src/typed.rs::TypedPullSupplier`
Trait mit `pull`/`try_pull`/`disconnect` Operations.

**Tests:** `typed::tests::{pull_supplier_try_pull_returns_queued_event,
pull_supplier_try_pull_returns_none_for_empty,
pull_supplier_disconnect_returns_error}`.

**Status:** done

### §2.6 Typed Event Channels (Architektur-Überblick)

**Spec:** §2.6, S. 2-16 — Konzept Typed-Channel.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §2.7 The CosTypedEventChannelAdmin Module

**Spec:** §2.7, S. 2-16 — IDL-Modul mit TypedEventChannel +
TypedConsumerAdmin/TypedSupplierAdmin + Typed-Proxies.

**Repo:** `typed.rs` `TypedEventChannel` mit `for_consumers()`/
`for_suppliers()`/`destroy()` + getrennte `TypedConsumerAdmin`
und `TypedSupplierAdmin` Strukturen + `TypedPullSupplier`-Trait
für den Pull-Proxy-Pfad.

**Tests:** Inline (siehe Sub-Sections).

**Status:** done — Channel + Admin-Splits + Proxies live.

#### §2.7.1 The TypedEventChannel Interface

**Spec:** §2.7.1 — `for_consumers`/`for_suppliers`/`destroy` analog
§2.3.1.

**Repo:** `typed.rs::TypedEventChannel::{for_consumers, for_suppliers,
destroy, is_destroyed}`.

**Tests:** `typed::tests::{typed_event_channel_for_consumers_returns_admin,
typed_event_channel_for_suppliers_returns_admin,
destroy_disables_dispatch}`.

**Status:** done

#### §2.7.2-§2.7.5 Typed Admin/Proxy Interfaces

**Spec:** §2.7.2-§2.7.5 — TypedConsumerAdmin, TypedSupplierAdmin,
TypedProxyPushConsumer, TypedProxyPullSupplier.

**Repo:** `typed.rs::{TypedConsumerAdmin, TypedSupplierAdmin}` mit
`register_consumer`/`register_pull_supplier`/`try_pull`/Counter-
Operationen. `TypedProxyPushConsumer` ist via existierender
`TypedPushConsumer`-Trait abgedeckt; `TypedProxyPullSupplier`
über den neuen `TypedPullSupplier`-Trait.

**Tests:** `typed::tests::{typed_consumer_admin_register_count_roundtrip,
typed_supplier_admin_register_count_roundtrip,
typed_supplier_admin_try_pull_returns_first_available}`.

**Status:** done — Admin-/Proxy-Split implementiert.

### §2.8 Composing Event Channels and Filtering

**Spec:** §2.8, S. 2-20 — Channel-Komposition + Filter-Pattern
(informativ; Filter ist in eigener Spec CosNotification).

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §2.9 Policies for Finding Event Channels

**Spec:** §2.9, S. 2-20 — Naming-/Trader-Service-Verweise zur
Channel-Discovery (informativ).

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## §3 Lightweight Event Service

### §3.1 Platform Independent Model (PIM)

#### §3.1.1 Overview

**Spec:** §3.1.1 — Lightweight-Profile-Beschreibung.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

#### §3.1.2 The CosLightweightEventComm Package

**Spec:** §3.1.2 — Lightweight-Variante von §2.1 (kein Typed-Event,
kein Multi-Consumer, reduzierte API).

**Repo:** `crates/corba-cos-event/src/typed.rs::lightweight::
{PROFILE_NAME, is_lightweight}` als Profile-Marker; das
Lightweight-Subset nutzt denselben Push-Pfad wie
`TypedPushConsumer`, ohne disconnect-Lifecycle (per
Channel-`destroy()`).

**Tests:** `typed::tests::{lightweight_profile_names_match_spec,
lightweight_is_lightweight_recognizes_both_profiles}`.

**Status:** done

#### §3.1.3 The CosLightweightEventChannel Package

**Spec:** §3.1.3 — Lightweight-Variante von §2.3.

**Repo:** `crates/corba-cos-event/src/typed.rs::lightweight::
CHANNEL_PROFILE_NAME` Marker; Channel-Lifecycle über
`TypedEventChannel::destroy()` ohne separates
ConsumerAdmin/SupplierAdmin-Sub-Profil (Profile-Subset von §2.7.1).

**Tests:** Cross-Ref §3.1.2.

**Status:** done

### §3.2 Platform Specific Model: CORBA Service

#### §3.2.1 Overview

**Spec:** §3.2.1 — PSM-Marker.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

#### §3.2.2 CosEventChannelAdmin Module (PSM)

**Spec:** §3.2.2 — IDL-PSM, identisch zu §2.3.

**Repo:** identisch §2.3 (`channel.rs`).

**Tests:** Inline.

**Status:** done — siehe §2.3.

#### §3.2.3 CosEventComm Module (PSM)

**Spec:** §3.2.3 — IDL-PSM, identisch zu §2.1.

**Repo:** identisch §2.1 (`comm.rs`).

**Tests:** Inline.

**Status:** done — siehe §2.1.

---

## ZeroDDS-spezifische Bridges (kein OMG-Item)

### CosEvent → DDS-DCPS-Topic Bridge

**Spec:** kein OMG-Spec-Item; ZeroDDS-spezifische Migrations-Schicht,
die EventChannel-Push-Events in DDS-DCPS-Topics übersetzt.

**Repo:** `crates/corba-cos-event/src/channel.rs` mit DDS-Hook
(per `forward_event` über `corba-dds-bridge`).

**Tests:** Inline.

**Status:** done — informativ; nicht spec-pflichtig.

---

## Audit-Status

25 done / 0 partial / 0 open / 10 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-corba-cos-event` — 23 Tests grün, 0
failed:
* `channel::tests::push_fan_out_to_multiple_consumers`,
  `destroy_disconnects_proxies`, `double_connect_yields_already_connected`,
  `pull_supplier_dequeues_events`, `pull_supplier_disconnect_propagates`.
* `comm::tests::any_event_round_trip`,
  `connect_error_variants_distinct`,
  `push_increments_counter_until_disconnect`.
* `typed::tests::consumer_count_reflects_registrations`,
  `dispatch_reaches_registered_consumers`,
  `disconnected_consumers_are_skipped_in_count`,
  `unknown_repo_id_dispatches_to_zero`.
