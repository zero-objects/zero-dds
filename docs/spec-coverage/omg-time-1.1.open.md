# OMG Time Service 1.1 — Open + Partial Items

— keine offenen Items.

Alle §2.2 / §2.3 / §2.4 partials wurden in
`crates/corba-ccm/src/time_psm.rs` mit spec-konformen IDL-PSM-Typen
(`TimerEventT`, `EventStatus`, `TimeType`), Exception-Hierarchie
(`TimerError`), `TimerEventHandler` mit status/time_set/set_timer/
set_data, `event_time`-Operation und `TimerEventServiceFacade::register`
(PushConsumerLike-Adapter) abgedeckt.
