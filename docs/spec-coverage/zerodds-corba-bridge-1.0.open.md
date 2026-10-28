# zerodds-corba-bridge-1.0 — offene Punkte

Aggregat aus `zerodds-corba-bridge-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §2 zerodds-corba-bridged CLI

**Status:** `partial` — Exit-Code 5 (NameService) entfällt, solange die NameService-Registrierung nicht gebaut ist (§4.1); Exit 2 und 3 sind im Code gesetzt, aber ohne eigenen Test.

## §4.1 IOR-Generation IIOP/SSLIOP/Components

**Status:** `partial` — `TAG_CODE_SETS` und `TAG_SSL_SEC_TRANS` gebaut, `catior` liest beide; kein `TAG_ORB_TYPE` (keine OMG-vergebene ORB-Kennung); `publish_to_naming: true` wird abgelehnt (NameService nicht gebaut).

## §4.3 Request-Reply-Mapping CORBA→DDS

**Status:** `partial` — `NO_PERMISSION`, `NO_IMPLEMENT`, `TRANSIENT` bei DDS-Schreibfehler, `MARSHAL` und `CancelRequest` sind gebaut, aber ohne eigenen Test; `correlation_id` außer `request_id` wird abgelehnt.

## §4.7 LocateRequest/Reply OBJECT_HERE/UNKNOWN/FORWARD

**Status:** `partial` — `OBJECT_HERE` und `UNKNOWN_OBJECT` sind gebaut, aber im Daemon ohne Test (`tests/cross_vendor.rs::locate_request_handler_classifies_known_and_unknown` prüft `src/locate.rs` der Bibliothek, nicht den Daemon); `OBJECT_FORWARD` ist nicht gebaut.

## §5.2 Type-Discovery via idl-rust Codegen

**Status:** `partial` — die IDL-Generierung pro Mapping und die Exception-Variante im Reply-Typ sind nicht gebaut; die Bridge kennt den IDL-Typ nicht und reicht die Argument-Bytes durch.

## §6 DDS-QoS → CORBA-Behavior Map

**Status:** `partial` — Reliability und Durability gehen an die DDS-Entities; ein Replay pro Request-Hash für `TRANSIENT_LOCAL`, Lifespan, Deadline und Partition sind nicht gebaut.

## §7.2 CSIv2 SAS_ContextElement + GSSUP

**Status:** `open` — CSIv2 ist im Daemon nicht gebaut. Die Bausteine `crates/corba-csiv2/` und `crates/corba-dds-bridge/src/csiv2_wire.rs` sind nicht an den Daemon angebunden. Plan: den `SAS_ContextElement` aus dem ServiceContext jedes Requests lesen, GSSUP gegen eine Benutzerdatei prüfen und das Subject in die ACL aus §7.3 geben.

## §8.1 Strukturiertes JSON-Logging

**Status:** `partial` — eine JSON-Zeile pro Ereignis auf stderr mit `level`, `event` und den Ereignisfeldern, gefiltert nach `log_level`; `timestamp` fehlt, und kein Test prüft die Form einer Zeile.

## §8.3 OTLP-Spans

**Status:** `partial` — der Exporter wird aus der Umgebung gestartet, aber `bridge_daemon.rs` erzeugt keine Spans.

## §9.1 Startup-Sequence

**Status:** `partial` — der NameService-Rebind entfällt, solange `publish_to_naming` nicht gebaut ist; die übrigen Schritte laufen in der Reihenfolge der Spec.

## §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Status:** `partial` — der Zweig nach 30 s (`drain_expired`) hat keinen Test.

## §10 RTPS-Peer + TAO/JacORB/omniORB/Ice-Java

**Status:** `partial` — gegen omniORB gemessen. TAO, JacORB und Ice-Java sind nicht gemessen; der Rest steht im `open`-Item §10/§12.

## §12.1 Unit-Tests pro Modul

**Status:** `partial` — der Daemon hat acht Unit-Tests in einem Modul; die Aufteilung der Klausel mit ≥ 5 Tests pro Modul gibt es nicht, und `csiv2` ist nicht gebaut.

## §12.3 Multi-Vendor cross_vendor.rs

**Status:** `partial` — die DDS-Seite hinter der Bridge ist ZeroDDS; ein Cyclone-DDS-Backend und ein TAO-Client sind nicht gemessen. Cross-Ref: das `open`-Item darunter.

## §10/§12 Cross-Vendor-Harness — TAO, JacORB, Ice-Java, Cyclone

**Status:** `open` — gemessen ist omniORB mit einem ZeroDDS-Backend (`tests/omniorb_e2e.rs`). Plan: dieselben Fälle mit einem TAO- und einem JacORB-Client aus `crates/corba-interop/competitors/` und mit einem Cyclone-DDS-Backend nach dem Layout aus §4.3 auf dem Linux-Testhost fahren.
