# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-corba-cos-event`-Crate.

### Spec-Referenzen

- **OMG CosEventService 1.4** (`formal/2004-10-02`): §1.5 (CosEventComm), §1.6 (CosEventChannelAdmin), §2 (CosTypedEventComm + CosTypedEventChannelAdmin).

### Public-API

**Comm (`comm`-Modul):**
- `AnyEvent`, `Disconnected`, `ConnectError`.
- `PushConsumer`, `PushSupplier`, `PullConsumer`, `PullSupplier` Trait-Surfaces mit Disconnect-Operations.

**Channel (`channel`-Modul):**
- `EventChannel`, `ConsumerAdmin`, `SupplierAdmin`.
- `ProxyPushConsumer`, `ProxyPushSupplier`, `ProxyPullConsumer`, `ProxyPullSupplier`.

**Typed (`typed`-Modul):**
- `TypedEventChannel`, `TypedPushConsumer`, `TypedPushSupplier`.

### Implementierung

`EventChannel` haelt zwei Admins (`ConsumerAdmin` + `SupplierAdmin`); jeder Admin verwaltet eine Liste von Proxy-Consumers/Suppliers. Beim `connect_*_supplier()` / `connect_*_consumer()` werden die Caller-Trait-Objects in den Proxy gespeichert; Disconnect emittiert `Disconnected` an die Gegenseite.

Typed-Variante erlaubt, dass das Caller-Interface ein konkretes IDL-Interface ist (statt opaquer `AnyEvent`); typed_consumer/supplier-Operations liefern den Caller-spezifischen Operations-Dispatcher.

`#![cfg_attr(not(feature = "std"), no_std)]`, `#![forbid(unsafe_code)]`. `extern crate alloc`.

### Architektur

- **Layer:** 8 (CORBA-Stack, Tier-A).
- **Dependencies (in):** keine.
- **Dependents (out):** (vorgesehen) Caller-Daemon mit konkreten Channel-Konfigurationen.
- **Feature-Flags:** `std` (default), `alloc` (via std).

### Stabilitaet

- Public-API: RC1-stabil.
- Trait-Signaturen: durch OMG-Spec fixiert.
