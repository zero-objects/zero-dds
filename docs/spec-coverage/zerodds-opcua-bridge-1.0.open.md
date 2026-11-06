# zerodds-opcua-bridge-1.0 — offene Punkte

Aggregat aus `zerodds-opcua-bridge-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §2 zerodds-opcua-bridged CLI

**Status:** `partial` — geprüft sind `--bind` (auch in `=`-Form), `--domain`, wiederholtes `--idl` und seine Darstellung als `+opcua.idl=…`, `-h` und die Ablehnung eines unbekannten Flags. Ohne Test: die Exit-Codes von `--check-config`, `--config`, `--gateway-config`, `--log-level`, `--metrics`, `--version` und der Vorrang von `--set` vor den Dateien.

## §4.3 Part-14 UADP

**Status:** `partial` — Publisher/Subscriber in beide Richtungen und der statische Schlüssel sind gebaut und geprüft; die Schlüsselrotation über `GetSecurityKeys`/SKS fehlt (§4.3.1).

## §4.3.1 SKS-Schlüsselrotation

**Status:** `open` — der Daemon holt keine Schlüssel über `GetSecurityKeys` und rotiert sie nicht; ein neuer Schlüssel braucht einen Neustart (§9.3).

## §5.1 TypeObject/TypeLookup → SerializedTypeSupport

**Status:** `partial` — der IDL-Pfad ist geprüft; `resolve_discovered` hat keinen Test; die eigene `getTypes`-Abfrage fehlt (§5.1.1).

## §5.1.1 getTypes-Abfrage für einen unbekannten Typ

**Status:** `open` — ein Topic, dessen Typ weder aus `opcua.idl` noch aus der Registry der Runtime kommt, wird nicht gebrückt; der Daemon fragt den Typ nicht über TypeLookup an.

## §5.3 Typformen-Abdeckung

**Status:** `partial` — die genannten Formen sind abgebildet und geprüft; Array/Sequence von Struct/Union, `bitset`, `fixed<P,S>` und `float128` haben keinen Live-Wert (§5.3.1); ein neuer `map`-Schlüssel wird nicht als Knoten angelegt (§5.3.2).

## §5.3.1 Live-Wert für Array/Sequence von Struct/Union, bitset, fixed, float128

**Status:** `open` — `dynamic_value.rs::flatten` meldet diese Formen mit Pfad und lässt den Knoten ohne Wert; es fehlt die Abbildung auf eine Array-Variable bzw. einen passenden Wert.

## §5.3.2 Neuer map-Schlüssel zur Laufzeit

**Status:** `open` — `AddressSpace` und `PublishedDataSet` bauen ihre Knoten- und Feldmenge einmal aus dem Table-9-Default; ein später auftauchender Schlüssel bekommt keinen Knoten.

## §5.4 Historical Access (§9.3.4)

**Status:** `partial` — Markierung, Retention, Raw-Range, AtTime, Continuation-Point und die Ablehnung von `is_read_modified` sind geprüft. Ohne Test: der Ladezeit-Fehler für einen `dds_topic` ohne `dds_to_ua_connection` (`crates/opcua-bridge/src/daemon/server.rs`, „names no dds_to_ua_connection topic“).

## §6.2 OPC UA → DDS

**Status:** `partial` — beide Mechanismen sind gebaut und geprüft; die Rückmeldung eines fehlgeschlagenen DDS-Writes an den Client fehlt (§6.2.1).

## §6.2.1 Rückmeldung eines fehlgeschlagenen DDS-Writes

**Status:** `open` — der Weg nach DDS bleibt ein Poll; ein Fehler beim DDS-Write wird nur geloggt.

## §8.1 Logging

**Status:** `open` — die Meldungen stehen als `eprintln!` in `crates/opcua-bridge/src/daemon/server.rs`; kein Test prüft `log_level` oder eine Meldung.

## §8.2 Prometheus-Metriken

**Status:** `open` — `crates/opcua-bridge/src/daemon/server.rs::{BridgeMetrics, serve_metrics}` ist gebaut; kein Test liest den Endpunkt.

## §9.1/§9.2 Start + Shutdown

**Status:** `partial` — Start und `shutdown()` im Prozess sind geprüft. Ohne Test: die Signalbehandlung (SIGTERM/SIGINT) und der Exit-Code 0 des Binaries.

## §9.3 SIGHUP

**Status:** `open` — der Reload-Thread in `crates/opcua-bridge/src/daemon/server.rs::start_with_reload` ist gebaut; kein Test sendet SIGHUP.

## §11 Paket, systemd-Unit, Beispielkonfiguration, Handbuchseiten

**Status:** `partial` — die Beispielkonfiguration lädt durch den echten Parser. Ohne Test: Unit, Handbuchseiten und Paketeinträge; `tools/traceability/tests/deployment_layout.rs` und `tools/traceability/tests/bridge_config_manpages.rs` führen den OPC-UA-Daemon nicht in ihrer Bridge-Liste.
