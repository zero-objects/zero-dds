# `check_create_participant` — kanonische ZeroDDS-Semantik

**Status:** kanonisch · **Quelle:** DDS-Security 1.2 §8.4.2.9.3 ·
**Code:** `crates/security-permissions/src/governance.rs::Governance::check_create_participant`

Diese Notiz hält die **verbindliche** ZeroDDS-Semantik für die
Participant-Create-Zugriffskontrolle fest, damit der zweimal aufgetretene
Fehlgriff (Table 63 als *topology-only* lesen) nicht erneut entsteht.

## Regel (grant-basiert, §8.4.2.9.3)

`check_create_participant(permissions, domain_id, now)` konsultiert **sowohl**
die Governance-Topologie **als auch** den Permissions-Grant:

| Bedingung | Ergebnis |
|---|---|
| Keine `<domain_rule>` deckt `domain_id` | **deny** |
| `enable_join_access_control = FALSE` | **allow** (offener Join, kein Grant nötig) |
| `enable_join_access_control = TRUE` | **allow gdw.** ein Grant existiert, der `now` gültig ist und dessen `<domains>` `domain_id` matcht |

**Folge:** Eine voll access-controlled Governance
(`enable_join_access_control=TRUE` + jedes Topic read+write-AC=TRUE) ist
**NICHT un-joinable**. Sie ist joinbar von jedem Participant, dessen
Permissions-Grant die Domain abdeckt — genau so behandeln **Cyclone DDS** und
**Fast DDS** sie (Beleg: ZeroDDS↔FastDDS `all-enc` 109/123 µs; SROS2-Full-
Lockdown lebt von genau diesem Pfad).

## Was NICHT gilt (häufiger Fehlgriff)

- **Table 63 NICHT topology-only lesen.** Die isolierte Tabelle-63-Lesart
  („allow nur, wenn ein Topic read/write-AC=FALSE *oder* join-AC=FALSE")
  ignoriert die Grant-Klausel aus §8.4.2.9.3 und macht full-AC fälschlich
  „un-joinable". Das ist falsch für konforme Peers.
- **OpenDDS' Selbst-Ablehnung ist nicht bindend.** OpenDDS
  (`AccessControlBuiltInImpl.cpp ~L281-348`) implementiert die topology-only
  Lesart und lehnt full-AC sogar gegen sich selbst ab. Das ist eine
  **OpenDDS-spezifische** Haltung, kein universelles Spec-Verdikt. ZeroDDS
  übernimmt sie nicht.

## Konfigurierbarkeit (keine „canned profiles")

Es gibt **keine** fest verdrahteten Governance-Profile. Der Create-Gate
arbeitet ausschließlich auf real geparstem XML:

- Governance via `zerodds_security_set_governance_path` → `parse_governance_xml`
- Permissions via `zerodds_security_set_permissions_path` → `parse_permissions_xml`
- Permissions-CA via `zerodds_security_set_permissions_ca_path`

`zerodds_runtime_create_secure` ruft `check_create_participant` mit den
geparsten Permissions auf. Jede beliebige Governance+Permissions-Kombination
ist damit abbildbar — wie bei Cyclone/Fast DDS.

## Historie

Ein früherer „Härtungs"-Pass hatte einen topology-only Reject in ZeroDDS
eingebaut (`Governance::is_domain_joinable` + `SharedSecurityGate::
is_domain_joinable` + create-deny-Gate), der full-AC unconditional ablehnte —
der dokumentierte Table-63-Revert-Fehler. Commit `f1c8eb3c` entfernt diesen
Strang vollständig; der grant-basierte `check_create_participant` ist der
**einzige** Create-Gate. Test: `check_create_participant_consults_permissions`.

Siehe auch: [`cross-vendor-secure-interop-matrix.md`](cross-vendor-secure-interop-matrix.md),
[`opendds-secure-matrix-closeout.md`](opendds-secure-matrix-closeout.md).
