# Security Advisories

Register für veröffentlichte Security-Advisories zu ZeroDDS. Prozess-seitig
flankiert von `SECURITY.md` im Repo-Root (Disclosure-Policy, PGP-Kontakt).

Kompatibilitaet: das Register ist so strukturiert, dass es spaeter als
CSAF- oder OpenSSF-konformer Feed exportierbar ist (Expansion-Era, CRA-
Compliance gemaess `docs/architecture/07_risks_and_strategy.md §5.3`).

## Nummerierungs-Schema

```
ZERODDS-YYYY-NNNN
```

Beispiel: `ZERODDS-2026-0001`. Nummern werden fortlaufend vergeben, parallel
werden externe CVE-IDs beantragt sobald verfuegbar.

## Pflichtinhalte je Advisory

1. **ID** (ZERODDS- + ggf. CVE-)
2. **Severity** (CVSS 3.1 Base-Score + Vektor)
3. **Betroffene Versionen** und fixed-in
4. **Betroffene Crates/Module**
5. **Beschreibung** der Schwachstelle
6. **Impact** und Attack-Szenario
7. **Workarounds** (falls Upgrade nicht sofort moeglich)
8. **Credits** (Reporter)
9. **Timeline** (Report, Fix, Release, Disclosure)
10. **Referenzen** (Commits, PRs, externe Reports)

## Index

| ID | Severity | Betroffene Crates | Status | Veroeffentlicht |
|---|---|---|---|---|
| — | _(keine Advisories)_ | — | — | — |
