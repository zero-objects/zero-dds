# `zerodds-corba-cos-event`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-corba-cos-event/badge.svg)](https://docs.rs/zerodds-corba-cos-event)

OMG CosEventService 1.4 (`formal/2004-10-02`) voller Stack —
Push/Pull-Modell + EventChannelAdmin + TypedEvent.
`no_std + alloc`, `forbid(unsafe_code)`. Safety classification:
**STANDARD**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| OMG CosEventService 1.4 (`formal/2004-10-02`) | §1.5 (CosEventComm), §1.6 (CosEventChannelAdmin), §2 (CosTypedEventComm + CosTypedEventChannelAdmin) |

## Was ist drin

- **`AnyEvent`** — opaque Event-Body-Container.
- **`PushConsumer` / `PushSupplier` / `PullConsumer` / `PullSupplier`**
  — §1.5 Trait-Surfaces mit Disconnect-Operations.
- **`EventChannel` / `ConsumerAdmin` / `SupplierAdmin`** — §1.6
  Channel-Admin.
- **`ProxyPushConsumer` / `ProxyPushSupplier` / `ProxyPullConsumer` /
  `ProxyPullSupplier`** — §1.6 Proxies.
- **`TypedEventChannel` / `TypedPushConsumer` / `TypedPushSupplier`**
  — §2 Typed-Variant.
- **`Disconnected` / `ConnectError`** — Spec-§1.5-Errors.

## Schichten-Position

Layer 8 — CORBA-Stack (Tier-A). Caller-Layer (Daemon o.ae.)
konstruiert konkrete Channel-Instanzen, registriert Suppliers und
Consumers, und treibt die Connect/Disconnect-Lifecycle.

## Quickstart

```rust
use zerodds_corba_cos_event::{AnyEvent, EventChannel};

let channel: EventChannel = EventChannel::new();
let _consumer_admin = channel.for_consumers();
let _supplier_admin = channel.for_suppliers();
```

## Feature-Flags

| Feature | Default | Zweck |
|---------|---------|-------|
| `std` | ✅ | Standard-Library. |
| `alloc` | ✅ (via std) | `Vec` / `String` / `Arc`. |

`no_std`-fahig: `default-features = false, features = ["alloc"]`.

## Stabilitaet

`1.0.0-rc.1`. Public-API + Trait-Surfaces sind RC1-stabil; Spec-
Aenderungen erfordern Major-Bump.

## Tests

```bash
cargo test -p zerodds-corba-cos-event
```

23 Unit-Tests grün.

## Lizenz

Apache-2.0. Siehe [LICENSE](../../LICENSE).

## Siehe auch

- [`docs/release/rc1-reviews/corba-cos-event.md`](../../docs/release/rc1-reviews/corba-cos-event.md) — RC1-Review.
