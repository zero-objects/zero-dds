# Track Post-B — Security-Compliance (Detail)

**Status:** 📋 backlog (post-1.0)

**Trigger:** EU Cyber Resilience Act greift Dezember 2027 für
Open-Source. Vorher sinnvoll wenn Pilot-Kunden Compliance fordern.

## Items

1. **Threat-Model** (1 PW) — `docs/security/threat-model.md` mit STRIDE
   pro Layer + DDS-spezifische Threat-Categories (Discovery-Spoofing,
   Wire-MITM, ACL-Bypass).
2. **CRA-Compliance-Doku** (2 PW) — `docs/compliance/cra/` mit Vendor-
   Statement, Vulnerability-Disclosure-Process, SBOM-policy. Pflicht
   ab 2027.
3. **SLSA-3 Build-Provenance** (2 PW) — Sigstore-Cosign zusätzlich zu
   minisign, in-toto-Attestations, SLSA-3-konformer CI-Workflow
   (hermetische Builds, signed checkpoints).
4. **FIPS-140-3-Mode** (1 PW) — opt-in feature in security-crypto die
   nur FIPS-validated cipher-suites zulässt. Doc + Test-Suite gegen
   FIPS-Validation.
5. **Pen-Test** (extern beauftragt) — keine Self-Estimation, Budget in
   €€€-Range. Trigger-Pull wenn Customer-Demand das rechtfertigt.
6. **Authenticode-Cert für Windows** — separater Cost-Issue. Trigger:
   wenn Windows-Adoption 5%+ ist.

## Acceptance

- Threat-Model published, Cross-Reference im SECURITY.md
- CRA-Statement als PDF auf zerodds.org/compliance/cra-self-assessment.pdf
- SLSA-Provenance-Attestations für jeden Release-Artifact downloadable
- FIPS-Mode-Test grün auf nicht-FIPS-Hardware (rustls hat das ready)

## Out-of-Scope

- Common-Criteria EAL-Zertifizierung (Aerospace-Level, separater Track
  unter Industry-Verticals)
- ISO 27001 für ZeroDDS-as-Org (Org-Level, nicht Software-Level)
