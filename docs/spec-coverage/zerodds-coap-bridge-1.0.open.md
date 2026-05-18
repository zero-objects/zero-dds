# `zerodds-coap-bridge` v1.0 — Open + Partial Items

Aggregat aus `zerodds-coap-bridge-1.0.md`. Nicht von Hand pflegen — vor jedem
Audit-Lauf löschen und aus dem Hauptfile neu generieren.

## Open

— keine. Alle vormals offenen Items im RC1-Cluster-A/B/C-Closeout
2026-05-07 geschlossen oder per Decision-Record als `n/a (rejected)`
klassifiziert.

## Partial

— keine. Alle vormals partial-Items im RC1-Cluster-A/B/C-Closeout
2026-05-07 nach `done` migriert (Block-Wise + QoS-Map + ACL +
Metrics + Shutdown).

## Decision-Records (`n/a (rejected)`)

### §7.1 DTLS coaps:// + Cipher-Suites

**Status:** `n/a (rejected)` — Pure-Rust-DTLS-Stack 2026 nicht audit-
ready (analoge Argumentation zu ADR-0007 OSCORE; Zero-Touch-DTLS-Pfad
in RC1 deferred). Auth+ACL ist via Cluster-B-Wireup über CoAP-Vendor-
Option 65000 (Application-Auth-Token) voll abgedeckt; siehe
`crates/coap-bridge/src/daemon/security.rs` +
`crates/coap-bridge/tests/security_e2e.rs`.

### §7.2 OSCORE (RFC 8613)

**Status:** `n/a (rejected)` — siehe
`docs/adr/0007-coap-oscore-rejected-rc1.md`. OSCORE ist im 2026-IoT-
Markt nischig; Cloud-IoT (AWS/Azure) und Industrial-Edge-Stacks nutzen
einheitlich (D)TLS. Volle COSE-Stack-Implementation ohne Customer-Pull
liefert keinen Hebel; Spec-Schema bleibt formell normativ-optional.
