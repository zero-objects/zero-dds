# zerodds-corba-cos-notify

OMG **CosNotification 1.1** (formal/2004-10-11) — the structured-event
notification service, successor to CosEvent.

In-memory model, `forbid(unsafe_code)`. `event`, `comm` and `qos` build
with `no_std + alloc` (`--no-default-features --features alloc`);
`channel` and `filter` hold shared state behind `std::sync::Mutex` and
need the `std` feature (default).

- **event** §2.2 — `StructuredEvent` / `EventType` / `Property` (values as CORBA `any`). `no_std + alloc`.
- **comm** §3 — structured push/pull consumer/supplier traits + `NotifyPublish`/`NotifySubscribe`. `no_std + alloc`.
- **channel** §4 — `EventChannelFactory` / `EventChannel` / `ConsumerAdmin` /
  `SupplierAdmin` / structured-proxy hierarchy (push fan-out + pull queue, QoS `MaxQueueLength`). `std`.
- **filter** §5 — `Filter` / `ConstraintExp` / `FilterFactory` (event-type match with `"*"` wildcard). `std`.
- **qos** §2.5 — QoS / admin property bag. `no_std + alloc`.

Live wiring over GIOP (analogous to the CosEvent `EventBus` e2e in
`zerodds-corba-interop`) happens in the interop layer.
