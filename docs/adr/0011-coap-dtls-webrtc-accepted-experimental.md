# ADR 0011 — CoAP DTLS 1.2 via webrtc-dtls (opt-in, experimental)

- **Status:** Accepted (2026-06-12)
- **Supersedes:** the "DTLS DEFER" recommendation in
  `docs/dossiers/dtls-pure-rust-dossier.md` §5.2
- **Related:** ADR 0010 (OSCORE accepted) — the object-security path

## Context

`crates/coap-bridge` covers RFC 7252 CoAP. §9 / §7.1 specify `coaps://`
transport security over **DTLS** (RFC 6347 / 9147). The workspace previously
declined to wire a DTLS record layer: the dossier found no audit-ready,
ecosystem-standard pure-Rust DTLS server library in 2026, and a placeholder
in `coap-bridge/Cargo.toml` stated "a pure-Rust DTLS lib … is not approved".
OSCORE (ADR 0010) shipped as the object-security alternative.

The owner revisited this: spec-completeness is the project's mission, and
"not independently audited" is **not**, by itself, a reject reason for an
**opt-in** profile. ZeroDDS is itself a young project asking users for trust;
refusing a mature-enough pure-Rust DTLS on purity grounds is inconsistent.

A candidate, **hptls** (`seceq/hptls`), was evaluated and **rejected** on
objective grounds, independent of maturity:

- **No license grant.** The repository contains no `LICENSE`/`LICENSE-MIT`/
  `LICENSE-APACHE` files; `Cargo.toml` only carries the string
  `license = "MIT OR Apache-2.0"` and a template-placeholder
  `repository = "https://github.com/yourusername/hptls"`. Without the actual
  grant text the code cannot be lawfully taken as a dependency.
- **git-only** (not on crates.io) — would break `cargo publish` of any
  crate that depends on it (the workspace ships 97/97 crates on crates.io).
- **Own unaudited crypto** (`hpcrypt`: from-scratch RSA via `num-bigint`,
  curves, PQC ML-KEM/ML-DSA/SLH-DSA), git-only in a second repository.

## Decision

Add an **opt-in, experimental** DTLS 1.2 transport to `coap-bridge` behind a
new `dtls` feature, built on **`webrtc-dtls`** (the `webrtc-rs` DTLS stack):

- crates.io-published (v0.12), **MIT/Apache-2.0 with real license files**,
  ~3.9M downloads, used in production WebRTC stacks — legally and
  publishably clean.
- `dtls_transport.rs` provides `DtlsCoapServer`, `DtlsCoapClient`, and
  `DtlsCoapSession`: a UDP + DTLS handshake (self-signed cert, extended
  master secret required) carrying CoAP messages (existing codec) as DTLS
  application data.
- `webrtc-util` pinned to `^0.11` to share webrtc-dtls 0.12's `Conn` /
  `Listener` trait instance.

The feature is **not** part of `default`. The published `no_std` codec core,
the default build, and the OSCORE path are untouched.

## Honesty constraints (non-negotiable)

- **Labelled experimental.** Module docs + the feature comment state plainly:
  DTLS 1.2 (not 1.3), webrtc-dtls not independently audited for non-WebRTC
  use, `insecure_skip_verify` is the dev/test default. Not a hardened
  production default.
- **OSCORE remains the recommended constrained-object-security path**
  (proxy-traversal, no handshake) — DTLS is for callers that specifically
  need transport-channel security + a certificate handshake.

## Consequences

- `coaps://` is now achievable with a real pure-Rust DTLS handshake + an
  e2e test (`dtls_coap_e2e.rs`: handshake + CoAP GET → 2.05 Content).
- Opting into `dtls` pulls `tokio` + `webrtc-dtls` + `webrtc-util` + `rustls`
  (all crates.io). Callers without the feature pay nothing.
- Future improvement (non-blocking): DTLS 1.3 once a pure-Rust stack matures;
  certificate-verification wiring beyond self-signed for production use.
