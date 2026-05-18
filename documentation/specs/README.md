# ZeroDDS Vendor Specifications

The ZeroDDS Project publishes the following formal Vendor
Specifications, authored in the stylistic pattern of the OMG DDS Core
Specifications (numbered clauses, normative-vs-informative annexes,
RFC 2119 keywords, conformance profiles).

## v1.0-beta1 (2026-04-29)

### [DDS-AMQP 1.0 Beta 1](releases/v1.0-beta1/dds-amqp-1.0-beta1.pdf)

Bridge Specification for OMG DDS over OASIS AMQP 1.0. Defines three
conformance profiles (Endpoint, Bridge, Codec), a normative type-system
mapping, three body-encoding modes (Pass-Through, JSON, AMQP-Native)
and a compliance test suite.

Tag: `spec-dds-amqp-1.0-beta1`

### [DDS-TS 1.0 Beta 1](releases/v1.0-beta1/dds-ts-1.0-beta1.pdf)

TypeScript Platform-Specific Model for OMG IDL 4.2. Defines
IDL-to-TypeScript type mapping, a decorator-runtime
(`@zerodds/types`), and reserves a future WASM-Bindings profile.

Tag: `spec-dds-ts-1.0-beta1`

## Bridge specifications (1.0)

The seven protocol-bridge daemons each have a published Vendor
Specification:

| Bridge | Spec |
|---|---|
| WebSocket (RFC 6455) | `zerodds-ws-bridge-1.0.md` |
| MQTT 5.0 | `zerodds-mqtt-bridge-1.0.md` |
| CoAP (RFC 7252 + 7641 + 7959) | `zerodds-coap-bridge-1.0.md` |
| AMQP 1.0 daemon | `zerodds-amqp-bridge-daemon-1.0.md` |
| gRPC (HTTP/2) | `zerodds-grpc-bridge-1.0.md` |
| CORBA 3.3 (GIOP/IIOP) | `zerodds-corba-bridge-1.0.md` |
| ROS-2 (REP-2007/2008/2009) | `zerodds-ros2-bridge-1.0.md` |

Plus

- `zerodds-ffi-loader-1.0.md` — cross-language ABI loader spec
- `zerodds-deployment-1.0.md` — Linux / macOS / Windows production
  deployment

## Repository

The reference implementation and test harness are hosted at
[github.com/zero-objects/zero-dds](https://github.com/zero-objects/zero-dds).

## Build

LaTeX sources live under [`v1.0-beta1/`](v1.0-beta1/) and similar
release directories. Build via `tectonic` (single-binary TeX engine):

```bash
make -C documentation pdfs       # all stations + vendor specs
```

PDFs land in `documentation/dist/`.

---

Copyright © 2026 ZeroDDS Project. Apache License 2.0.
