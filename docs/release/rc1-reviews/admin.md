# RC1 Review — `admin`

> **Reference:** `docs/release/RC1_GUARDRAILS.md` §1.1–§1.13.
> **Layer:** Tools
> **Public-Strategy:** 🌐 public

## 1 Purpose

Admin CLI for ZeroDDS: domain inspector, QoS validator, discovery snapshot.

## 2 RC1 Definition-of-Done Checklist

| § | Item | Status |
|---|---|---|
| 1.1 | Cargo.toml metadata complete (name, version=1.0.0-rc.1, edition=2024, rust-version=1.88, license, description, repository, homepage, documentation, readme, keywords ≤5, categories ≤5, authors, publish flag) | ✅ |
| 1.2 | `main.rs` / `lib.rs` crate header with safety class + spec ref or "Internal Tool" marker + layer + public API or CLI subcommands | ✅ |
| 1.3 | `README.md` | ✅ |
| 1.4 | `CHANGELOG.md` with `[1.0.0-rc.1]` entry | ✅ |
| 1.5 | Public-API audit — no accidental glob re-exports | ✅ |
| 1.5b | Coherence audit — public items either CONNECTED or documented as OPTIONAL-HOOK | ✅ |
| 1.7 | Forbidden-token sweep §2.1 (hard-forbidden), §2.1b (sprint markers), §2.1c (date markers) | ✅ |
| 1.8 | SPDX license header on every `*.rs` file | ✅ |
| 1.10 | Review doc (this file) | ✅ |
| 1.11 | Tracker entry → `✅ rc1-ready` | ✅ |
| 1.12 | Public mirror under `github/tools/admin/` (only for 🌐) | 🌐 |
| 1.13 | Spec-conformance audit — 0 inline-deferral markers | ✅ |

## 3 Findings

None open. Pre-existing findings (if any) tracked in
`docs/release/RC1_FINDINGS.md`.

## 4 Sign-off

Crate is RC1-ready. New CLI subcommands or arguments are additive minor
bumps; flag removals require a major bump.
