# `amqp-dds-endpoint`

> DDS-AMQP 1.0 Endpoint daemon: synchronous std-only TCP/TLS server
> bridging AMQP 1.0 brokers to DDS topics.

[![Crates.io](https://img.shields.io/crates/v/amqp-dds-endpoint.svg)](https://crates.io/crates/amqp-dds-endpoint)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)

ZeroDDS tool implementing the OMG DDS-AMQP 1.0 Endpoint Profile
(§2.1). Combines `zerodds-amqp-bridge` (Wire-Codec) and
`zerodds-amqp-endpoint` (State-Machine) into a multi-threaded
listener that maps AMQP 1.0 sender/receiver links onto DDS
DataWriter/DataReader pairs.

## Spec mapping

| Spec | Section |
| --- | --- |
| OMG DDS-AMQP 1.0 | §2.1 Endpoint Profile, §9.2 XML config, §10.1 TLS |
| OASIS AMQP 1.0 | Frame, Performatives, SASL, Sections |

## Safety classification

**TOOLING** — server-daemon, not safety-critical runtime code.

## Quickstart

```bash
amqp-dds-endpoint --listen 0.0.0.0:5672
amqp-dds-endpoint --config /etc/zerodds/endpoint.xml
echo 'secret' | amqp-dds-endpoint --hash-password alice >> /etc/zerodds/amqp-users
```

A configuration file (Spec §9.2). An unknown element or attribute is refused
with its line.

```xml
<dds-amqp xmlns="http://www.zerodds.org/dds-amqp/v1.0">
  <endpoint name="warehouse">
    <listen-uri>amqp://0.0.0.0:5671</listen-uri>
    <limits max-frame-size="65536"/>
    <tls enabled="true" cert="/etc/zerodds/amqp-cert.pem" key="/etc/zerodds/amqp-key.pem"
         ca="/etc/zerodds/amqp-ca.pem" require-client-cert="false"/>
    <sasl>
      <mechanism>SCRAM-SHA-256</mechanism>
      <mechanism>PLAIN</mechanism>
      <credential-store-uri>file:///etc/zerodds/amqp-users</credential-store-uri>
    </sasl>
  </endpoint>
</dds-amqp>
```

`--listen` beats `<listen-uri>`.

## SASL (Spec §2.1 Cl. 6/7, §10.2)

* `PLAIN` only on a TLS connection; a `PLAIN` init on plaintext gets outcome
  `auth` and the connection closes.
* `SCRAM-SHA-256` and `PLAIN` are checked against the credential store: one
  line per user, `<user> SCRAM-SHA-256$<iterations>:<salt>$<StoredKey>:<ServerKey>`
  (RFC 5803, base64). It holds no passwords; `--hash-password` writes a line.
* `EXTERNAL` is the client certificate (`mtls:<SHA-256 of the DER>`), offered
  only with `require-client-cert="true"`.
* `ANONYMOUS` only when named, or when there is no credential store. Without
  it, a client that skips SASL gets the SASL header back and is closed
  (AMQP 1.0 §5.3.1).
* Without an `<sasl>` block: `ANONYMOUS`, and `EXTERNAL` for a client with a
  certificate. With a store and no mechanism list: `SCRAM-SHA-256`, and
  `PLAIN` on TLS.

A mechanism that cannot work as configured (`PLAIN` without TLS or store,
`SCRAM-SHA-256` without a store, `EXTERNAL` without client certificates) stops
the start with the reason.

SIGTERM and SIGINT stop the accept loop.

## Features

* `default = ["tls"]` — rustls TLS termination (Spec §10.1); the binary needs it.

## Stability

Public CLI: `--config`, `--listen`, `--hash-password`, `--help`.
Breaking changes require a major version bump.

## Build & test

```bash
cargo build -p amqp-dds-endpoint
cargo test  -p amqp-dds-endpoint
```

## Links

* [`crates/amqp-bridge/`](../../crates/amqp-bridge/) — Wire codec.
* [`crates/amqp-endpoint/`](../../crates/amqp-endpoint/) — State machine.
* [DDS-AMQP 1.0 Spec](https://www.omg.org/spec/DDS-AMQP/1.0/).
* [`CHANGELOG.md`](CHANGELOG.md).

## Licence

Apache-2.0. See [`LICENSE`](../../LICENSE).
