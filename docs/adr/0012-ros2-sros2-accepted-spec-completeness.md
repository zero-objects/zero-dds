# 0012 — ROS-2 SROS2 enclaves + permissions XML — accepted, implemented (supersedes 0008)

- **Status:** accepted (supersedes [0008](0008-ros2-sros2-rejected-rc1.md))
- **Date:** 2026-06-12
- **Context:** `crates/security-runtime`, `crates/rmw-zerodds-shim`,
  spec `zerodds-ros2-bridge-1.0` §7.1 + §7.2,
  spec completeness programme ("extra mile") + ROS clusters C6/C7/C8

## Context

ADR 0008 classified SROS2 enclaves (§7.1) and the permissions XML mapping
(§7.2) as `rejected/n/a` for RC1 — justified solely by missing market pull
("87% of ROS-2 robots without SROS2"; DDS-Security 1.2 covers the same
threat model, only with a different config file). ADR 0008 even explicitly
anticipated the follow-up ("until RC2 catches up"). That was an
**RC1 scoping decision**, not a technical impossibility.

The spec completeness programme explicitly overrides this scoping decision
(see [0010](0010-coap-oscore-accepted-spec-completeness.md)):
optional spec profiles are a **differentiating feature**, not a
reason to reject ("other vendors do not have it" does not count). With that, the
alternative 1 rejected in ADR 0008 (full SROS2 enclave mapping) is now the
chosen one — **implement fully, no stub, no hidden TODO**.

## Decision

The SROS2 `sros2-keystore` enclave is loaded as a complete mapping layer on top of
DDS-Security 1.2: the six enclave files (identity CA, cert, key,
permissions CA, `governance.p7s`, `permissions.p7s`) are mapped to a
`SecurityProfile`; governance/permissions pass byte-exact through
the existing CMS path in `crates/security-permissions` (DDS-Security 1.2
§9.4). "Secure by default" is wired up at the rmw shim via the env variable `ZERODDS_SECURITY_DIR`
(+ `ROS_DOMAIN_ID`).

## Architecture

| Layer | Status | Verification |
|---|---|---|
| **`SecurityProfile::from_enclave_dir`** (sros2-keystore → 6 paths → `SecurityProfile`) | ✅ done | `security-runtime` `enclave_dir_resolves_all_sros2_filenames` |
| **`SecurityProfile::from_env`** (C7 secure-by-default via `ZERODDS_SECURITY_DIR` + `ROS_DOMAIN_ID`) | ✅ done, superseded | The enclave is now the runtime setting `security.enclave_dir` (env `ZERODDS_SECURITY_DIR`, file or `--set`); `security_ffi::configured_enclave_dir` + `SecurityProfile::from_enclave_dir` replace `from_env`. Unset → plain, set-but-invalid → hard error, as before |
| **Error mapping** (missing material → IO error that names it) | ✅ done | `enclave_dir_missing_cert_is_io_naming_cert` |
| **Governance + permissions (§9.4)** via CMS `.p7s` | ✅ done | `crates/security-permissions` (CMS-verified + parsed) |
| **rmw shim wireup** (set → DDS-Security participant, set-but-failed → hard NULL) | ✅ done | `rmw-zerodds-shim` `shim_cli_e2e` (`ZERODDS_SECURITY_DIR` path) |

## Alternatives

1. **Stay with ADR 0008 (rejected)** — rejected: contradicts the
   spec completeness mandate (optional profiles are features).
2. **SROS2 as a stub** — rejected (as in 0008): fakes security.
3. **Implement fully as a mapping layer on top of DDS-Security 1.2** — chosen.

## Consequences

Positive:
- §7.1 + §7.2 change from `rejected` to `implemented` — clean spec status.
- ROS-2 deployments with active SROS2 start against ZeroDDS without manual
  enclave translation: one env variable is sufficient (`ZERODDS_SECURITY_DIR`).
- The migration use case ROS-2 → ZeroDDS becomes drop-in for the security path.

## References

- REP-2018 — Application of Security to ROS 2
- DDS-Security 1.2 §9.4 (live in `crates/security-*`)
- `crates/security-runtime/src/profile.rs` — `from_enclave_dir` / `from_env`
- `crates/rmw-zerodds-shim/src/lib.rs` — `ZERODDS_SECURITY_DIR` wireup
- `docs/spec-coverage/zerodds-ros2-bridge-1.0.md` §7.1, §7.2
- [0008](0008-ros2-sros2-rejected-rc1.md) — previous (superseded) decision
- [0010](0010-coap-oscore-accepted-spec-completeness.md) — same policy (OSCORE)
