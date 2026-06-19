# DDS-Security / SROS2

← [Zurück zur Übersicht](index.md)

## Der Schmerz

DDS-Security (SROS2) ist mächtig, aber fragil aufzusetzen (**22 Reports**). Der wiederkehrende Fehler ist, dass **das Einschalten von Security Discovery oder Kommunikation bricht**, oft mit wenig Diagnostik, plus echte Korrektheits-Lücken:

- Discovery-Encryption *und* Topic-Level-Protection zusammen zu aktivieren hindert Endpoints am Matchen.
- Security mit dem Micro-XRCE-DDS-Agent lässt Discovery komplett scheitern.
- Security-Enclave-Overrides greifen nicht — `ros2 node list` gibt mit Security nur System-Topics zurück.
- Unvollständige Privilege-Inheritance hat tatsächliche Sicherheitslücken produziert.

### Jüngstes Beispiel

**[Fast-DDS#5753 — „Discovery Matching fails when discovery_protection_kind=ENCRYPT and topic-level protection are both enabled"](https://github.com/eProsima/Fast-DDS/issues/5753)** (2025-04-08). Zwei standardmäßige, unterstützte Security-Einstellungen kombiniert lassen Endpoints aufhören zu matchen — Security-Konfiguration als Discovery-Brecher.

### Referenzliste (jüngste zuerst)

| Datum | Quelle | Problem |
|---|---|---|
| 2025-04-08 | [Fast-DDS#5753](https://github.com/eProsima/Fast-DDS/issues/5753) | Discovery-Encryption + Topic-Protection → kein Match |
| 2025-03-13 | [Fast-DDS#5707](https://github.com/eProsima/Fast-DDS/issues/5707) | Security + Micro-XRCE-Agent → Discovery scheitert |
| 2024-08-07 | [ros2#1589](https://github.com/ros2/ros2/issues/1589) | Unvollständige Privilege-Inheritance → Vulnerability |
| 2024-05-08 | [sros2#306](https://github.com/ros2/sros2/issues/306) | Enclave-Override wirkungslos; nur System-Topics sichtbar |
| 2024-04-17 | [sros2#293](https://github.com/ros2/sros2/issues/293) | Node-Liste leer mit aktivierter Security |

## Wie ZeroDDS es löst

**Eine vollständige DDS-Security-1.2-Implementierung, getestet als Cross-Vendor-Matrix — sodass „Security an" eine getestete Konfiguration ist, keine Klippe.**

- **Volle DDS-Security 1.2.** Authentication-, Access-Control-, Cryptographic-, Logging- und Data-Tagging-Built-in-Plugins sind alle implementiert (inkl. CRL und einer Conformance-Matrix). Security ist keine nachträgliche Schicht, die mit Discovery desynct — sie ist Teil des auditierten Stacks.
- **Secured Discovery ist eine Regressions-Zelle, keine Überraschung.** Genau die „encrypt discovery + protect topics"-Kombinationen, die anderswo brechen, sind Zellen in der Cross-Vendor-Security-Matrix von ZeroDDS, gegen Cyclone, Fast DDS und OpenDDS exerziert. Der Secured-Handshake (Authentication, Key-Exchange, Secured-SEDP/Data) ist e2e-getestet.
- **Profile, kein rohes Plumbing.** Ein `SecurityProfile` plus ein `runtime_create_secure`-FFI-Entry-Point schaltet Security über eine definierte Oberfläche ein, statt Enclaves und Governance-/Permissions-XML von Hand zusammenzubauen, deren Fehler still scheitern.
- **Memory-safe by construction.** Die Privilege-/Parse-Pfade laufen in sicherem Rust mit expliziten Bounds — die Klasse von Memory-Safety-Vulnerabilities hinter ([ros2#1589](https://github.com/ros2/ros2/issues/1589)) ist im sicheren Kern nicht ausdrückbar.

> **Ehrlicher Status:** Secured-*Cross-Vendor*-Interop ist breit, aber noch nicht in jeder Zelle 100 % grün (z.B. werden spezifische OpenDDS-Secured-SEDP-Decode-Pfade noch geschlossen). Wo eine Secured-Zelle verifiziert ist, sagen wir es; die offenen Zellen werden getrackt, nicht versteckt.

## Warum es kein Schmerz mehr sein muss

Der Security-Cluster ist *Security-Konfiguration, die still Discovery bricht*. ZeroDDS implementiert die volle Spec und behandelt die gefährlichen Kombinationen als explizite Regressions-Tests über Vendoren, exponiert über eine Profile-API — sodass das Einschalten von Security ein unterstützter, getesteter Schritt ist statt eines separaten Debugging-Projekts.

## Selbst reproduzieren

```bash
# Secured-Runtime via Profile + FFI-Entry-Point; Cross-Vendor-Secured-Matrix-
# Harness exerziert encrypt-discovery + topic-protection-Kombinationen.
```

→ [Zurück zur Übersicht](index.md) · Weiter: [Performance](performance.md)
