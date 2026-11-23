# omg-time-1.1 — offene Punkte

Aggregat aus `omg-time-1.1.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §1.2 General Object Model — Service-Object pattern

**Status:** `partial` — der Dienst CosTime (TimeService, UTO, TIO) ist als CORBA-Objekt gebaut, mit einem GIOP-Client im Prozess geprüft (`time_svc_e2e`, GIOP 1.0 und 1.2, beide Bytereihenfolgen) und gegen omniORB 4.3.3: `omniorb_e2e` lief auf dem Linux-Testrechner in einem Debian-trixie-Container mit omniidl und libomniORB4, 3 von 3 grün (mit beiden `RemoteTimePolicy`-Werten, auch `absolute_time`); im Job `integration-bridges-vendor-corba` eingetragen, dort noch nicht gelaufen. Es fehlt der zweite Dienst als CORBA-Objekt: `TimerEventService` und `TimerEventHandler` haben keinen GIOP-Dispatch, `register` nimmt keine entfernte `PushConsumer`-Referenz, und ein Timer-Ereignis geht nicht als `any` mit TypeCode über GIOP hinaus.

## §1.2.1 Conformance Points (Basic + Timer Event)

**Status:** `partial` — CosTimerEvent ist als Rust-API vollständig, aber nicht als CORBA-Objekt über GIOP erreichbar (§1.2); CosTime ist als CORBA-Objekt erreichbar; die Aussage zu den Kriterien von App. A für den Basic Time Service steht in `crates/time-service/README.md` (§2.5, App. A).

## §2.5 Conformance Statement

**Status:** `partial` — der Timer Event Service ist als Rust-API vollständig, über GIOP nicht erreichbar (§1.2). Basic Time Service: erfüllt; die Aussage, wie die Kriterien von App. A behandelt werden, steht in `crates/time-service/README.md` („Secure time (Appendix A and B)"), und `secure_universal_time` liefert nur unter diesen Kriterien eine Zeit (§2.1.2, App. A).
