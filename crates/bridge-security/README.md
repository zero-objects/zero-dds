# zerodds-bridge-security

> Shared security layer for ZeroDDS bridge daemons (ws / mqtt / coap / amqp / grpc / corba):
> §7.1 TLS (rustls), §7.2 auth modes, §7.3 topic ACL.

[![Crates.io](https://img.shields.io/crates/v/zerodds-bridge-security.svg)](https://crates.io/crates/zerodds-bridge-security)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)

ZeroDDS component: Layer 5 (Bridges). Bundles three reusable building
blocks that any bridge daemon can plug in without having to write its
own crypto/auth logic.

## Spec mapping

| Spec document | Section |
| ------------- | ------- |
| ZeroDDS Bridge Spec 1.0 | §7.1 TLS |
| ZeroDDS Bridge Spec 1.0 | §7.2 auth modes |
| ZeroDDS Bridge Spec 1.0 | §7.3 topic ACL |

## Safety classification

**STANDARD** — security-relevant, but not safety-certified.
`#![forbid(unsafe_code)]` is set.

## Usage

```rust,no_run
use zerodds_bridge_security::{Acl, AclOp, AuthSubject};

let subj = AuthSubject::new("alice").with_group("publishers");
let acl = Acl::allow_all();
let _allowed = acl.check(&subj, AclOp::Write, "/topics/trade");
```

## Building blocks

* [`tls`] — `rustls 0.23` ServerConfig builder with PEM cert/key loader,
  optional client CA trust for mTLS, plus `RotatingTlsConfig` for
  SIGHUP hot reload.
* [`auth`] — auth modes `none|bearer|jwt|mtls|sasl` plus
  [`AuthSubject`] type with group memberships and free-form claims.
* [`acl`] — topic ACL with wildcard and group matching (read / write).
* [`connection`] — per-connection helpers: server-side TLS handshake,
  rustls `ClientConnector` builder, RotatingTlsConfig.
* [`ctx`] — `SecurityCtx` as an aggregate (auth + ACL + TLS), plus
  `authenticate` / `authorize` / `extract_mtls_subject`.

## Features

* `default = ["std"]` — standard library + heap allocator.
* `std` — required (rustls 0.23 needs std).

## Stability

`1.0.0-rc.1` — the public API is RC1-stable. Breaking changes require a
major bump. New auth modes or ACL operations are additive discriminants
and thus major-additive.

## Build & test

```bash
cargo build -p zerodds-bridge-security
cargo test -p zerodds-bridge-security
```

## License

Apache-2.0. See [LICENSE](../LICENSE).
