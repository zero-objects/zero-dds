# zerodds-corba-bridge-1.0 — offene Punkte

Aggregat aus `zerodds-corba-bridge-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §2 zerodds-corba-bridged CLI

**Status:** `partial` — Exit 3 (DDS) ist im Code gesetzt, aber ohne eigenen Test.

## §4.1 IOR-Generation IIOP/SSLIOP/Components

**Status:** `partial` — `TAG_CODE_SETS` und `TAG_SSL_SEC_TRANS` gebaut, `catior` liest beide; kein `TAG_ORB_TYPE` (keine OMG-vergebene ORB-Kennung). Den NameService-Bind prüfen `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_10_the_bridge_rebinds_into_omninames_and_unbinds_on_stop` und `crates/corba-dds-bridge/tests/jacorb_e2e.rs::corbabridge_10_the_bridge_rebinds_into_jacorbs_name_server_and_unbinds_on_stop`.

## §4.3 Request-Reply-Mapping CORBA→DDS

**Status:** `partial` — `NO_IMPLEMENT`, `TRANSIENT` bei DDS-Schreibfehler und `CancelRequest` sind gebaut, aber ohne eigenen Test; `correlation_id` außer `request_id` wird abgelehnt.

## §4.7 LocateRequest/Reply OBJECT_HERE/UNKNOWN/FORWARD

**Status:** `open` — `OBJECT_HERE` und `UNKNOWN_OBJECT` sind im Daemon gebaut (`crates/corba-dds-bridge/src/bridge_daemon.rs::serve_connection`, Zweig `LocateRequest`), aber ohne Test; `OBJECT_FORWARD` ist nicht gebaut.

## §5.2 Type-Discovery via idl-rust Codegen

**Status:** `partial` — die IDL-Generierung pro Mapping und die Exception-Variante im Reply-Typ sind nicht gebaut; die Bridge kennt den IDL-Typ nicht und reicht die Argument-Bytes durch.

## §6 DDS-QoS → CORBA-Behavior Map

**Status:** `partial` — Reliability und Durability gehen an die DDS-Entities; ein Replay pro Request-Hash für `TRANSIENT_LOCAL`, Lifespan, Deadline und Partition sind nicht gebaut.

## §7.2 CSIv2 SAS_ContextElement + GSSUP

**Status:** `open` — CSIv2 ist im Daemon nicht gebaut; `csiv2.enabled: true` wird abgelehnt (`crates/corba-dds-bridge/src/bridge_daemon.rs`, Test `corbabridge_3_what_the_daemon_does_not_build_is_refused`). Die Bausteine `crates/corba-csiv2/` und `crates/corba-dds-bridge/src/csiv2_wire.rs` sind nicht an den Daemon angebunden.

## §8.1 Strukturiertes JSON-Logging

**Status:** `open` — `crates/corba-dds-bridge/src/bridge_daemon.rs` (`log`, `event_level`, `level_rank`) schreibt eine JSON-Zeile pro Ereignis auf stderr mit `level`, `event` und den Ereignisfeldern, gefiltert nach `log_level`; `timestamp` fehlt, und kein Test prüft die Form einer Zeile.

## §8.3 OTLP-Spans

**Status:** `partial` — der Exporter wird aus der Umgebung gestartet, aber `bridge_daemon.rs` erzeugt keine Spans.

## §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Status:** `partial` — der Zweig nach 30 s (`drain_expired`) hat keinen Test.

## §10 RTPS-Peer + TAO/JacORB/omniORB/Ice-Java

**Status:** `partial` — gegen omniORB, JacORB und TAO gemessen; Ice-Java nicht.

## §12.1 Unit-Tests pro Modul

**Status:** `partial` — der Daemon hat acht Unit-Tests in einem Modul; die Aufteilung der Klausel mit ≥ 5 Tests pro Modul gibt es nicht, und `csiv2` ist nicht gebaut.

## §12.3 Multi-Vendor cross_vendor.rs

**Status:** `partial` — die DDS-Seite hinter der Bridge ist ZeroDDS; ein Cyclone-DDS-Backend ist nicht gemessen. Cross-Ref: das Item darunter.

## §10/§12 Cross-Vendor-Harness — TAO, JacORB, Ice-Java, Cyclone

**Status:** `partial` — omniORB, JacORB und TAO sind gemessen (§10); Ice-Java und ein Cyclone-DDS-Backend nach dem Layout aus §4.3 nicht.
