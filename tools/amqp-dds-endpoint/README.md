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
```

A minimal XML config file (per Spec §9.2):

```xml
<dds_amqp_endpoint>
  <endpoint name="warehouse">
    <listen_uri>amqp://0.0.0.0:5672</listen_uri>
    <limits><max_frame_size>65536</max_frame_size></limits>
    <tls enabled="false"/>
  </endpoint>
</dds_amqp_endpoint>
```

## Features

* `default = []` — pure-std, plaintext AMQP only.
* `tls` — enable rustls 0.23 TLS termination (Spec §10.1).

## Stability

`1.0.0-rc.1` — public CLI: `--config`, `--listen`, `--help`.
Breaking changes require a major version bump.

## Build & test

```bash
cargo build -p amqp-dds-endpoint
cargo test  -p amqp-dds-endpoint
cargo build -p amqp-dds-endpoint --features tls
```

## Links

* [`crates/amqp-bridge/`](../../crates/amqp-bridge/) — Wire codec.
* [`crates/amqp-endpoint/`](../../crates/amqp-endpoint/) — State machine.
* [DDS-AMQP 1.0 Spec](https://www.omg.org/spec/DDS-AMQP/1.0/).
* [`CHANGELOG.md`](CHANGELOG.md).

## Licence

Apache-2.0. See [`LICENSE`](../../LICENSE).
