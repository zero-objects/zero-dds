# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-corba-cos-event` crate.

### Spec references

- **OMG CosEventService 1.4** (`formal/2004-10-02`): §1.5 (CosEventComm), §1.6 (CosEventChannelAdmin), §2 (CosTypedEventComm + CosTypedEventChannelAdmin).

### Public API

**Comm (`comm` module):**
- `AnyEvent`, `Disconnected`, `ConnectError`.
- `PushConsumer`, `PushSupplier`, `PullConsumer`, `PullSupplier` trait surfaces with disconnect operations.

**Channel (`channel` module):**
- `EventChannel`, `ConsumerAdmin`, `SupplierAdmin`.
- `ProxyPushConsumer`, `ProxyPushSupplier`, `ProxyPullConsumer`, `ProxyPullSupplier`.

**Typed (`typed` module):**
- `TypedEventChannel`, `TypedPushConsumer`, `TypedPushSupplier`.

### Implementation

`EventChannel` holds two admins (`ConsumerAdmin` + `SupplierAdmin`); each admin manages a list of proxy consumers/suppliers. On `connect_*_supplier()` / `connect_*_consumer()`, the caller trait objects are stored in the proxy; disconnect emits `Disconnected` to the other side.

The typed variant allows the caller interface to be a concrete IDL interface (instead of an opaque `AnyEvent`); the typed_consumer/supplier operations return the caller-specific operation dispatcher.

`#![cfg_attr(not(feature = "std"), no_std)]`, `#![forbid(unsafe_code)]`. `extern crate alloc`.

### Architecture

- **Layer:** 8 (CORBA stack, tier A).
- **Dependencies (in):** none.
- **Dependents (out):** (planned) caller daemon with concrete channel configurations.
- **Feature flags:** `std` (default), `alloc` (via std).

### Stability

- Public API: RC1-stable.
- Trait signatures: fixed by the OMG spec.
