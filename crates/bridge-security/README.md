# zerodds-bridge-security

> Gemeinsamer Security-Layer für ZeroDDS Bridge-Daemons (ws / mqtt / coap / amqp / grpc / corba):
> §7.1 TLS (rustls), §7.2 Auth-Modes, §7.3 Topic-ACL.

[![Crates.io](https://img.shields.io/crates/v/zerodds-bridge-security.svg)](https://crates.io/crates/zerodds-bridge-security)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)

ZeroDDS-Komponente: Layer 5 (Bridges). Bündelt drei wiederverwendbare
Bausteine, die jede Bridge-Daemon einbinden kann, ohne ihre eigene
Crypto-/Auth-Logik schreiben zu müssen.

## Spec-Mapping

| Spec-Dokument | Abschnitt |
| ------------- | --------- |
| ZeroDDS Bridge-Spec 1.0 | §7.1 TLS |
| ZeroDDS Bridge-Spec 1.0 | §7.2 Auth-Modes |
| ZeroDDS Bridge-Spec 1.0 | §7.3 Topic-ACL |

## Safety-Klassifikation

**STANDARD** — sicherheitsrelevant, aber nicht safety-zertifiziert.
`#![forbid(unsafe_code)]` ist gesetzt.

## Verwendung

```rust,no_run
use zerodds_bridge_security::{Acl, AclOp, AuthSubject};

let subj = AuthSubject::new("alice").with_group("publishers");
let acl = Acl::allow_all();
let _allowed = acl.check(&subj, AclOp::Write, "/topics/trade");
```

## Bausteine

* [`tls`] — `rustls 0.23` ServerConfig-Builder mit PEM-Cert/Key-Loader,
  optionalem Client-CA-Trust für mTLS, plus `RotatingTlsConfig` für
  SIGHUP-Hot-Reload.
* [`auth`] — Auth-Modes `none|bearer|jwt|mtls|sasl` plus
  [`AuthSubject`]-Typ mit Group-Memberships und Free-Form-Claims.
* [`acl`] — Topic-ACL mit Wildcard- und Group-Match (Read / Write).
* [`connection`] — pro-Connection-Helper: TLS-Handshake-Server-Side,
  rustls-`ClientConnector`-Builder, RotatingTlsConfig.
* [`ctx`] — `SecurityCtx` als Aggregat (Auth + ACL + TLS), plus
  `authenticate` / `authorize` / `extract_mtls_subject`.

## Features

* `default = ["std"]` — Standard-Library + Heap-Allocator.
* `std` — Pflicht (rustls 0.23 braucht std).

## Stabilitaet

`1.0.0-rc.1` — Public-API ist RC1-stabil. Breaking-Changes erfordern
einen Major-Bump. Neue Auth-Modes oder ACL-Operationen sind additive
Diskriminanten und damit Major-additive.

## Build & Test

```bash
cargo build -p zerodds-bridge-security
cargo test -p zerodds-bridge-security
```

## Lizenz

Apache-2.0. Siehe [LICENSE](../LICENSE).
