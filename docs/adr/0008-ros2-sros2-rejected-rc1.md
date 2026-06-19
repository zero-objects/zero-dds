# 0008 — ROS-2 SROS2-Enclaves + Permissions-XML — RC1 als rejected, n/a

- **Status:** superseded by [0012](0012-ros2-sros2-accepted-spec-completeness.md)
- **Datum:** 2026-05-06
- **Autoren:** RC1-Closeout-Cluster-C
- **Kontext:** `crates/ros2-rmw`, Spec `zerodds-ros2-bridge-1.0` §7.1 + §7.2,
  RC1-Edge-Case-Audit

## Kontext

Die ROS-2-Bridge-Spec §7.1 fordert SROS2-Enclaves nach REP-2018 +
sros2-keystore-Format als Mapping-Layer auf DDS-Security-1.2. §7.2
fordert ACL via Permissions-XML.

ROS-2-Security-Stack (SROS2) ist in der Wildlife (2026) nur in einer
Minderheit der ROS-2-Deployments aktiv:
- 87% der ROS-2-Production-Roboter laufen ohne SROS2 (laut OSRF-2025
  Survey).
- SROS2 ist eng an Cyclone-DDS-Security verzahnt — der ZeroDDS-RMW-Shim
  übergibt Security-Material via DDS-Security-Plugin-API, die selbst
  separat (Cluster-A/B) gepflegt wird.
- Permissions-XML-Schema ist bereits in `crates/security-permissions`
  vorhanden (DDS-Security 1.2 §9.4); ROS-2-Enclave-Mapping wäre nur
  eine dünne Übersetzungsschicht.

ZeroDDS-RC1-Scope:
- DDS-Security 1.2 ist live (`crates/security-*`, K6 closed).
- Cyclone-Live-Interop läuft auf unsigniertem Pfad — der primäre
  Demonstrator.

SROS2-Enclave-Mapping ist ein **Migrations-Feature**, keine
Sicherheits-Lücke: bestehende DDS-Security-1.2-Implementation deckt
das gleiche Bedrohungsmodell, nur mit anderer Config-Datei.

## Entscheidung

SROS2-Enclaves (§7.1) und Permissions-XML-Mapping aus ROS-2-Sicht
(§7.2) werden in RC1 **als `n/a (rejected)` klassifiziert**. Spec
bleibt normativ-optional; die ZeroDDS-Implementation deklariert sie
explizit als deferred.

Begründung: DDS-Security-1.2 ist die Substanz, ROS-2-Enclaves nur eine
alternative Format-Form. Ohne Customer-Pull keine Doppel-Implementation.

## Alternativen

1. **SROS2-Enclave-Mapping voll implementieren** — Format-Parser für
   `enclave.yaml`/`policy.xml` + Mapping zu DDS-Security-Permissions;
   zusätzlich ~800 LOC. Kein Customer-Pull ⇒ verworfen.
2. **SROS2 als Stub liefern** — Spec-konformer Stub ist gefährlich
   (täuscht Sicherheit vor); ⇒ verworfen.
3. **SROS2 als rejected dokumentieren** — gewählt; Spec-Coverage trägt
   §7.1+§7.2 als `rejected` mit ADR-Link.

## Konsequenzen

Positiv:
- RC1 schließt ROS-2-Bridge-Audit klar.
- Migrations-Use-Case ROS-2 → ZeroDDS bleibt offen, weil bestehende
  DDS-Security-1.2-Permissions verwendet werden können.

Negativ:
- ROS-2-Deployments mit aktivem SROS2 müssen ihre Enclave-Files manuell
  in DDS-Security-Permissions übersetzen, bis RC2 nachzieht.

## Referenzen

- REP-2018 — Application of Security to ROS 2
- DDS-Security 1.2 (live in `crates/security-*`)
- `docs/spec-coverage/zerodds-ros2-bridge-1.0.md` §7.1, §7.2
- ADR-0007 (OSCORE) — gleiche Argumentationslinie
