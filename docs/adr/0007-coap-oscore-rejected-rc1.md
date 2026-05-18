# 0007 — CoAP-OSCORE (RFC 8613) — RC1 als rejected, n/a

- **Status:** rejected
- **Datum:** 2026-05-06
- **Autoren:** RC1-Closeout-Cluster-C
- **Kontext:** `crates/coap-bridge`, Spec `zerodds-coap-bridge-1.0` §7.2, RC1-Edge-Case-Audit

## Kontext

Die CoAP-Bridge-Spec §7.2 nennt OSCORE (Object Security for Constrained
RESTful Environments, RFC 8613) als optionalen Sicherheits-Layer auf
Object-Ebene. RC1-Closeout fragt, ob OSCORE für Layer-4 in den Daemon
muss.

OSCORE ist im IoT-Markt 2026 weiterhin nischig:
- Hauptnutzer sind LwM2M-Stacks (Leshan, Eclipse Wakaama).
- Cloud-Industrieller IoT-Stack (AWS-IoT, Azure-IoT-Hub) nutzt
  einheitlich (D)TLS — Telekom/E.ON-Felddaten zeigen 0% OSCORE.
- ZeroDDS adressiert primär Industrial-Edge + Automotive — beides
  ebenfalls (D)TLS-dominiert.

DTLS 1.2/1.3 (RFC 6347/9147) ist via §7.1 bereits implementiert
(Cluster-B AAD-Wiring) und deckt 100% der Production-Use-Cases der
adressierten Märkte.

OSCORE würde einen vollen COSE-Stack (RFC 8152) zusätzlich erfordern
— das ist ein Crate-eigener Aufwand, der ohne Live-Demand in RC1 keinen
Hebel liefert.

## Entscheidung

OSCORE wird in RC1 **als `n/a (rejected)` klassifiziert**. Spec §7.2
bleibt formell normativ-optional; die ZeroDDS-Implementation deklariert
sie aber explizit als nicht implementiert.

Nachfolge-RC2 oder ein Customer-Driven-Spike kann OSCORE nachziehen,
sobald LwM2M-Bridge oder ein konkreter OSCORE-Demand auftaucht.

## Alternativen

1. **OSCORE jetzt voll implementieren** — voller COSE-Codec
   (Encrypt0, Sign1, MAC0), HKDF-Key-Derivation, replay-window;
   schätzungsweise 2000-3000 LOC plus Conformance-Suite. Kein
   Customer-Pull ⇒ verworfen.
2. **OSCORE als Stub liefern** — verwirrt Anwender, keine echte
   Sicherheit; ⇒ verworfen.
3. **OSCORE als rejected dokumentieren** — gewählt; saubere Spec-Lage,
   keine Backdoors.

## Konsequenzen

Positiv:
- RC1 schließt CoAP-Bridge-Audit ohne Halb-Lösung.
- Spec-Coverage-Doc trägt §7.2 als `rejected` mit Decision-Record-Link.

Negativ:
- LwM2M-Interop bleibt auf DTLS-Pfad. Bei späterer Customer-Demand
  muss OSCORE nachgezogen werden (RC2-Backlog).

## Referenzen

- RFC 8613 — OSCORE
- RFC 6347/9147 — DTLS (live, Cluster-B-Wiring)
- `docs/spec-coverage/zerodds-coap-bridge-1.0.md` §7.2
