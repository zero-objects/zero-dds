# OMG CCM 4.0 — Open + Partial Items

Aggregat aus `omg-ccm-4.0.md`. Nicht von Hand pflegen — vor jedem
Audit-Lauf löschen und aus dem Hauptfile neu generieren.

## Open

— keine.

## Partial

— keine.

## Decision-Records (`n/a (rejected)`)

— keine.

Im Layer-8 Wire-up-Cleanup 2026-05-06 wurden alle ehemals-rejected
Items (Conformance-Punkte 3 / 6 / 8) auf `done` reklassifiziert via
Wire-up gegen vorhandene Infrastruktur:

* §2 CP3 — `PssSession::{begin_transaction, commit, rollback}` mit
  Pending-Buffer; tx-aware `store(pid, value)` / `remove(pid)` /
  `load(pid)`; `PssTxStatus`-Lifecycle (`NoTransaction → Active →
  Committed`/`RolledBack`).
* §2 CP6 — `corba-ccm-ejb::connector_bean::pss_session_for_bean`
  brueckt `ConnectorBean` an `PssSession`; liefert `PssBeanBinding`
  mit Tx-Status zum Bind-Zeitpunkt.
* §2 CP8 — `Orb::{with_interceptor_registry, with_messaging_policy,
  with_compression}` plus Reader-Methoden; konfiguriert den
  ORB-Singleton fuer alle drei Component-Specific-Erweiterungen
  (Cross-Ref `corba-3.3.md` §16/§17/§18).

Siehe Audit-Status-Footer in `omg-ccm-4.0.md` fuer Test-Counts.
