# dds-opcua-1.0 — offene Punkte

Aggregat aus `dds-opcua-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §10.1 Gateway Configuration Overview

**Status:** `open` — im Baum liegen die beiden XSD-Dateien nicht; `crates/opcua-gateway/src/xml.rs` validiert gegen kein Schema und liest eine eigene Syntax (Item §10.2).

## §10.2 XML Configuration

**Status:** `open` — `crates/opcua-gateway/src/xml.rs` (`parse_gateway_config`) liest eine eigene Syntax mit Wurzel `<zerodds_opcua_gateway>`, `<bridge>`, `<ua_to_dds_connection>` und `<dds_to_ua_connection>` und weist die Wurzel `<dds>` der Spec ab; die Tests in `xml.rs` prüfen diese eigene Syntax.
