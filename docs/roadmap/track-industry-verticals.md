# Track Post-C — Industry-Verticals (Detail)

**Status:** 📋 backlog (post-1.0)

**Trigger:** per Vertikale erste Pilot-Kunden mit Compliance-Forderung.
Niemals vorab investieren — die Doku-Last pro Vertikale ist erheblich.

## Verticals

### Automotive

- **AUTOSAR Adaptive Platform**: DDS ist offizielles Communication-
  Pattern in Adaptive AP. ZeroDDS-Bridge zu ARA::COM (DDS-AAP).
  Estimate: 4-6 PW.
- **AUTOSAR Classic**: Bridge zu RTE über CAN/FlexRay-Encapsulation.
  Estimate: 6-8 PW.
- **ISO 26262 (ASIL-B)**: Safety-Manual + Argument-Tree + Tool-
  Qualification-Doku. Estimate: 4 PW.

### Aerospace

- **DO-178C**: Tool-Qualification-Doku, MC/DC-Coverage-Reports, Safety-
  Manual. Estimate: 6-8 PW (signifikante Doku-Last).
- **ARP4754A** für Systems-Level. Estimate: 2-3 PW.

### Industrial Automation

- **IEC 61508 (SIL-2/3)**: Sicherheitsnachweis für Process-Control-
  Anwendungen. Estimate: 4-5 PW.
- **OPC-UA-Pub-Sub-Mapping**: Bridge zu OPC-UA-Pub/Sub-Spec
  (`zerodds-opcua-gateway` existiert teilweise schon, ausbauen).
  Estimate: 2 PW.

### Medical

- **IEC 62304**: Software-Lifecycle für medizinische Geräte. Estimate:
  3-4 PW.
- **MDR/IVDR-Compliance**: EU-Medical-Device-Regulation, Cross-Check
  mit CRA. Estimate: 2 PW.

### Defense

- **NATO AEP-2025** (DDS over MIL-STD-1553/Ethernet): Bridge wenn
  Compliance gefordert. Estimate: 4-6 PW.
- **IPMS** (Integrated Platform Management System): naval. Estimate:
  4 PW.

## Acceptance pro Vertikale

- Compliance-Doku-Set published unter `docs/compliance/<vertical>/`
- Pilot-Kunden-Statement
- Pro-Vertical-Test-Suite + CI-Job
- Falls applicable: Audit-Report von externem Auditor

## Out-of-Scope

- Wir werden keine Vertical-spezifische Hardware liefern (kein "ZeroDDS-
  AUTOSAR-ECU"-Produkt). Wir bleiben Software-only.
- Keine eigene Vertical-Sales-Force.
