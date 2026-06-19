# 0012 — ROS-2 SROS2-Enclaves + Permissions-XML — accepted, implemented (supersedes 0008)

- **Status:** accepted (supersedes [0008](0008-ros2-sros2-rejected-rc1.md))
- **Datum:** 2026-06-12
- **Kontext:** `crates/security-runtime`, `crates/rmw-zerodds-shim`,
  Spec `zerodds-ros2-bridge-1.0` §7.1 + §7.2,
  Spec-Completeness-Programm („extra Meile") + ROS-Cluster C6/C7/C8

## Kontext

ADR 0008 klassifizierte SROS2-Enclaves (§7.1) und Permissions-XML-Mapping
(§7.2) für RC1 als `rejected/n/a` — begründet rein mit fehlendem Markt-Pull
(„87% der ROS-2-Roboter ohne SROS2"; DDS-Security 1.2 deckt dasselbe
Bedrohungsmodell, nur mit anderer Config-Datei). ADR 0008 hat den Nachzug
sogar explizit antizipiert („bis RC2 nachzieht"). Das war eine
**RC1-Scoping-Entscheidung**, keine technische Unmöglichkeit.

Das Spec-Completeness-Programm überschreibt diese Scoping-Entscheidung
explizit (siehe [0010](0010-coap-oscore-accepted-spec-completeness.md)):
optionale Spec-Profile sind ein **Differenzierungs-Feature**, kein
Reject-Grund („andere Vendoren haben das nicht" zählt nicht). Damit ist die
in ADR 0008 verworfene Alternative 1 (voller SROS2-Enclave-Mapping) jetzt die
gewählte — **voll implementieren, kein Stub, kein versteckter TODO**.

## Entscheidung

Der SROS2-`sros2-keystore`-Enclave wird als vollständiger Mapping-Layer auf
DDS-Security 1.2 geladen: die sechs Enclave-Dateien (Identity-CA, Cert, Key,
Permissions-CA, `governance.p7s`, `permissions.p7s`) werden auf einen
`SecurityProfile` abgebildet; Governance/Permissions laufen byte-genau durch
den bestehenden CMS-Pfad in `crates/security-permissions` (DDS-Security 1.2
§9.4). „Secure by default" wird über die Env-Variable `ZERODDS_SECURITY_DIR`
(+ `ROS_DOMAIN_ID`) am rmw-Shim verdrahtet.

## Architektur

| Schicht | Status | Verifikation |
|---|---|---|
| **`SecurityProfile::from_enclave_dir`** (sros2-keystore → 6 Pfade → `SecurityProfile`) | ✅ done | `security-runtime` `enclave_dir_resolves_all_sros2_filenames` |
| **`SecurityProfile::from_env`** (C7 secure-by-default via `ZERODDS_SECURITY_DIR` + `ROS_DOMAIN_ID`) | ✅ done | `from_env` gibt `Ok(None)` bei unset, hard-error bei set-aber-invalid |
| **Fehler-Mapping** (fehlendes Material → benennendes IO-Error) | ✅ done | `enclave_dir_missing_cert_is_io_naming_cert` |
| **Governance + Permissions (§9.4)** über CMS `.p7s` | ✅ done | `crates/security-permissions` (CMS-verifiziert + geparst) |
| **rmw-Shim-Wireup** (set → DDS-Security-Participant, set-aber-failed → harter NULL) | ✅ done | `rmw-zerodds-shim` `shim_cli_e2e` (`ZERODDS_SECURITY_DIR`-Pfad) |

## Alternativen

1. **Bei ADR 0008 (rejected) bleiben** — verworfen: widerspricht dem
   Spec-Completeness-Mandat (optionale Profile sind Features).
2. **SROS2 als Stub** — verworfen (wie in 0008): täuscht Sicherheit vor.
3. **Voll implementieren als Mapping-Layer auf DDS-Security 1.2** — gewählt.

## Konsequenzen

Positiv:
- §7.1 + §7.2 wechseln von `rejected` zu `implemented` — saubere Spec-Lage.
- ROS-2-Deployments mit aktivem SROS2 starten gegen ZeroDDS ohne manuelle
  Enclave-Übersetzung: eine Env-Variable genügt (`ZERODDS_SECURITY_DIR`).
- Migrations-Use-Case ROS-2 → ZeroDDS wird drop-in für den Security-Pfad.

## Referenzen

- REP-2018 — Application of Security to ROS 2
- DDS-Security 1.2 §9.4 (live in `crates/security-*`)
- `crates/security-runtime/src/profile.rs` — `from_enclave_dir` / `from_env`
- `crates/rmw-zerodds-shim/src/lib.rs` — `ZERODDS_SECURITY_DIR`-Wireup
- `docs/spec-coverage/zerodds-ros2-bridge-1.0.md` §7.1, §7.2
- [0008](0008-ros2-sros2-rejected-rc1.md) — vorherige (superseded) Entscheidung
- [0010](0010-coap-oscore-accepted-spec-completeness.md) — gleiche Policy (OSCORE)
