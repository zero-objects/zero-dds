# `zerodds-corba-cos-event`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-corba-cos-event/badge.svg)](https://docs.rs/zerodds-corba-cos-event)

OMG CosEventService 1.4 (`formal/2004-10-02`) full stack —
push/pull model + EventChannelAdmin + TypedEvent.
`no_std + alloc`, `forbid(unsafe_code)`. Safety classification:
**STANDARD**.

## Spec mapping

| Spec | Section |
|------|-----------|
| OMG CosEventService 1.4 (`formal/2004-10-02`) | §1.5 (CosEventComm), §1.6 (CosEventChannelAdmin), §2 (CosTypedEventComm + CosTypedEventChannelAdmin) |

## What's inside

- **`AnyEvent`** — opaque event-body container.
- **`PushConsumer` / `PushSupplier` / `PullConsumer` / `PullSupplier`**
  — §1.5 trait surfaces with disconnect operations.
- **`EventChannel` / `ConsumerAdmin` / `SupplierAdmin`** — §1.6
  channel admin.
- **`ProxyPushConsumer` / `ProxyPushSupplier` / `ProxyPullConsumer` /
  `ProxyPullSupplier`** — §1.6 proxies.
- **`TypedEventChannel` / `TypedPushConsumer` / `TypedPushSupplier`**
  — §2 typed variant.
- **`Disconnected` / `ConnectError`** — spec §1.5 errors.

## Layer position

Layer 8 — CORBA stack (tier A). The caller layer (daemon or similar)
constructs concrete channel instances, registers suppliers and
consumers, and drives the connect/disconnect lifecycle.

## Quickstart

```rust
use zerodds_corba_cos_event::{AnyEvent, EventChannel};

let channel: EventChannel = EventChannel::new();
let _consumer_admin = channel.for_consumers();
let _supplier_admin = channel.for_suppliers();
```

## Feature flags

| Feature | Default | Purpose |
|---------|---------|-------|
| `std` | ✅ | Standard library. |
| `alloc` | ✅ (via std) | `Vec` / `String` / `Arc`. |

`no_std`-capable: `default-features = false, features = ["alloc"]`.

## Stability

`1.0.0-rc.1`. Public API + trait surfaces are RC1-stable; spec
changes require a major bump.

## Tests

```bash
cargo test -p zerodds-corba-cos-event
```

23 unit tests green.

## License

Apache-2.0. See [LICENSE](../../LICENSE).