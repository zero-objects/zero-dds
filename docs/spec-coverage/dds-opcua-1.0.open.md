# DDS-OPCUA Gateway 1.0 — Open + Partial Items

Aggregat aus `dds-opcua-1.0.md`. Nicht von Hand pflegen — vor jedem
Audit-Lauf löschen und aus dem Hauptfile neu generieren.

## Open

— keine.

## Partial

### §2 Conformance Multi-Punkt

**Status:** `partial` — alle vier Conformance-Points abgedeckt
(Type-System + GDS-Mapping + Service-Sets + Subscription +
Historical), expliziter Multi-Punkt-Conformance-Marker am Crate-
Niveau noch nicht ausgewiesen. Aufwand 0.25 PW.

### §9.2 DDS Type System Mapping (Aggregated/Collection-Recursion)

**Status:** `partial` — Datenmodell + Scalar-Mapping done; volle
Aggregated/Collection/Nested-Type-Recursion zu OPC-UA-AddressSpace-
Builder ist Caller-Layer. Aufwand 0.5-1 PW.

## Decision-Records (`n/a (rejected)`)

— keine.
