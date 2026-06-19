# zerodds-corba-cos-notify

OMG **CosNotification 1.1** (formal/2004-10-11) — the structured-event
notification service, successor to CosEvent.

In-memory model (`no_std + alloc`, `forbid(unsafe_code)`):

- **event** §2.2 — `StructuredEvent` / `EventType` / `Property` (values as CORBA `any`).
- **comm** §3 — structured push/pull consumer/supplier traits + `NotifyPublish`/`NotifySubscribe`.
- **channel** §4 — `EventChannelFactory` / `EventChannel` / `ConsumerAdmin` /
  `SupplierAdmin` / structured-proxy hierarchy (push fan-out + pull queue, QoS `MaxQueueLength`).
- **filter** §5 — `Filter` / `ConstraintExp` / `FilterFactory` (event-type match with `"*"` wildcard).
- **qos** §2.5 — QoS / admin property bag.

Live wiring over GIOP (analogous to the CosEvent `EventBus` e2e in
`zerodds-corba-interop`) happens in the interop layer.
