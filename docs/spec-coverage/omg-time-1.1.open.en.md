# OMG Time Service 1.1 — Open + Partial Items

— no open items.

All §2.2 / §2.3 / §2.4 partials are covered in
`crates/corba-ccm/src/time_psm.rs` with spec-conformant IDL-PSM types
(`TimerEventT`, `EventStatus`, `TimeType`), the exception hierarchy
(`TimerError`), `TimerEventHandler` with status/time_set/set_timer/
set_data, the `event_time` operation and `TimerEventServiceFacade::register`
(a PushConsumerLike adapter).
