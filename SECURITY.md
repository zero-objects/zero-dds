# Security Policy

## Supported Versions

| Version | Supported |
|---|---|
| `1.0.0-rc.x` | Yes — Release Candidate, security fixes via point releases |
| `< 1.0.0-rc.1` | No — pre-release builds, please upgrade |

Once `1.0.0` ships, the support matrix expands to "current minor" plus
"current minor − 1" with critical-only fixes for the previous minor.

## Reporting a Vulnerability

**Do not open a public GitHub issue for security vulnerabilities.**

Report privately via one of:

- **GitHub Security Advisory** —
  <https://github.com/zero-objects/zero-dds/security/advisories/new>
  (preferred; private until coordinated disclosure)
- **Email** — `security@zerodds.org`
- **GPG-encrypted email** — fingerprint published at
  <https://zerodds.org/.well-known/security.txt>

Please include:

- Affected crate(s) and version(s)
- Reproduction steps or proof-of-concept
- CVSS 3.1 vector if you can compute one
- Your preferred attribution (or "anonymous")

## Disclosure Process

1. **Acknowledgement** within 72 hours (5 business days maximum).
2. **Triage** — Security-Lead confirms impact and CVSS within 7 days.
3. **Fix development** — coordinated patch on a private branch, with
   regression tests.
4. **Embargo** — coordinated public-disclosure date agreed with reporter,
   typically 14–90 days depending on severity and affected ecosystems.
5. **Release** — patched version published to crates.io and packaging
   channels; CVE assigned via GitHub Security Advisories or
   MITRE direct.
6. **Advisory** — published under
   `internal/security/advisories/` and announced via GitHub Releases.

## Hardening Guidance

ZeroDDS provides a hardened baseline by default:

- All `unsafe` blocks carry `// SAFETY:` justifications, lint-enforced.
- Foundation crates have zero external dependencies.
- TLS 1.3 only via `rustls 0.23` (memory-safe TLS stack); no OpenSSL FFI.
- Authentication modes: bearer, JWT-RS256, mTLS, SASL-PLAIN; choice is
  deployment-config not code-coupled.
- DDS-Security 1.2 plugins (Auth/AC/Crypto/Logging/Tagging) implement
  the OMG-mandated cryptographic suites.

Production deployments should additionally:

- Enable DDS-Security plugins via `Participant` QoS (not default-on, by
  spec).
- Pin TLS certificates and rotate via SIGHUP (`bridge-security`'s
  `RotatingTlsConfig`).
- Restrict bridge-daemon ACLs by topic and group.
- Run bridge daemons under unprivileged users with systemd hardening
  (see `packaging/linux/systemd/`).

## Security Audits

Layer-3 (DDS-Security) and Layer-5 (bridge-security) are the highest-
priority audit targets. Independent third-party audit reports are
published under `internal/security/audits/` once available.

## Bug Bounty

No formal bounty program in `1.0.0-rc.x`. Significant findings are
acknowledged in release notes and the `SECURITY.md` Hall of Fame
once established.
