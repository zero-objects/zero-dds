# RC1 Review — `zerodds-security-permissions`

> **Layer:** 4. **Reviewer:** Claude. **Public-Strategy:** 🌐 public.

## 1 Purpose

DDS-Security 1.1 §9.4 Builtin-Access-Control: Permissions/Governance-XML-Parser + S/MIME-CMS-Signatur-Verifier + Wildcard-Topic-Match + Delegation-Chain + PSK-Profile.

## 3 Content-Inventur

8 src-Files, **5618 LOC**, 136 Unit-Tests + 3 Integration-Suites grün.

### 3.4 Coherence-Audit

| Public-Item | Spec | External Refs | Klassifikation |
|---|---|---|---|
| `PermissionsAccessControl` | DDS-Security 1.1 §9.4 | `security-runtime`, `dcps` (cfg security) | CONNECTED |
| `xml::*` | §10.4.1 | intern + `dcps`-Tests | CONNECTED |
| `governance::*` | §9.4.1.1 | intern + `security-runtime` | CONNECTED |
| `signature::{XmlSignatureVerifier, NoOpVerifier, EnvelopeCheckVerifier, open_signed_permissions}` | §10.4.1.1 + RFC 5751 | intern; `NoOpVerifier` ist explizit Dev-Helper | CONNECTED |
| `cms::{CmsVerifier, CmsError}` | RFC 5751/5652/5280 | end-user-Production-Pfad | CONNECTED |
| `topic_match::topic_match` | §9.4.1.3 | intern + ggf. end-user | CONNECTED |
| `delegation_check::*` | ZeroDDS-Extension (Architektur §09) | `security-runtime` | CONNECTED |
| `psk_access::*` | DDS-Security 1.2 §10.8 | end-user | CONNECTED |

Ergebnis: **0 ❌-Klassen**.

## 6 Cleanup-Findings

- Forbidden-Token-Sweep: 0.
- Sprint-Marker pre: 25+ Treffer (`WP 4.2-a/b/c/d`, `WP 4H-h`, `WP 4H-j-b/d/f/h`). Post: **0**.
- No-op-Sweep: 0.
- SPDX in 8 src-Files post.

## 7 Cleanup-Actions

1. **F-SECURITY-PERMISSIONS-1** ✅ (Sprint-Marker-Massensweep): bulk-sed-Cleanup von 25 WP-Markers in `governance.rs`/`plugin.rs`/`xml.rs`/`cms.rs`. lib.rs in Guardrails §1.2-Form mit Modul-Aufzaehlung. signature.rs Header umformuliert ("WP 4.2-b" → "Dev-Helper-Trait, produktiver Verifier in `cms`-Modul"). delegation_check.rs Header de-sprintet.
2. **F-SECURITY-PERMISSIONS-2** ✅ (MVP-Comment in cms.rs): "fuer den MVP matchen wir nicht hierauf" → fachliche Beschreibung "first-cert-fallback ist Spec-konform fuer single-signer-Pfade".
3. SPDX in 8 src-Files.
4. Cargo.toml-Metadata + `publish=true`.
5. README + CHANGELOG mit voll Public-API-Aufzaehlung + ZeroDDS-Extension-Namespace-Doc.

## 10 Tests + Lints + Doc-Build

```
cargo test           ✅ 136 + integration suites
cargo clippy --tests -- -D warnings  ✅
cargo fmt -- --check ✅
zerodds-lint check   ✅
```

## 11 RC1-DoD

Alle 13 Punkte; **No-op 0 Treffer**.

## 12 Sign-off

`1.0.0-rc.1`. Reviewer Claude.
