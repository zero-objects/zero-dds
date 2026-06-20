# Component / crate map

ZeroDDS is split into ~120 crates. Every crate has its own README
with a short purpose statement. This chapter groups them by layer.

## Foundation

| Crate | Purpose |
|---|---|
| `zerodds-foundation` | GUID, SN, time, CRC, pool buffers, RCU cell, observability |
| `zerodds-cdr` | OMG CDR / XCDR1 / XCDR2 encoder + decoder |
| `zerodds-types` | Type representation, TypeObject (XTypes 1.3) |
| `zerodds-qos` | All QoS policies, compatibility helpers |

## RTPS + Discovery

| Crate | Purpose |
|---|---|
| `zerodds-rtps` | Wire types, submessages, fragmentation, reliable writer/reader, history cache |
| `zerodds-discovery` | SPDP / SEDP / WLP, security-aware variants |

## DCPS

| Crate | Purpose |
|---|---|
| `zerodds-dcps` | DomainParticipant, Publisher, Subscriber, DataWriter, DataReader, runtime event-loop |
| `zerodds-rpc` | Request/Reply pattern over DDS |

## Security

| Crate | Purpose |
|---|---|
| `zerodds-security` | SPI traits + token types |
| `zerodds-security-pki` | X.509 / PKI handshake (3-way) |
| `zerodds-security-permissions` | Governance + Permissions XML |
| `zerodds-security-crypto` | AES-GCM transforms (ring-backed; runtime HW detect) |
| `zerodds-security-rtps` | SRTPS submessage wrapping |
| `zerodds-security-keyexchange` | Volatile key-exchange topic |
| `zerodds-security-logging` | Logging plugin |
| `zerodds-security-runtime` | Glue between SPI and DCPS |

## Transport

| Crate | Purpose |
|---|---|
| `zerodds-transport` | Trait + Locator |
| `zerodds-transport-udp` | UDP socket + multicast |
| `zerodds-transport-tcp` | TCP framing |
| `zerodds-transport-shm` | POSIX shared-memory ring |
| `zerodds-transport-uds` | UNIX-domain sockets |
| `zerodds-transport-tsn` | Time-Sensitive Networking |

## Bridges

| Crate | Purpose |
|---|---|
| `zerodds-coap-bridge` | CoAP gateway |
| `zerodds-websocket-bridge` | WebSocket gateway |
| `zerodds-mqtt-bridge` | MQTT-5 gateway |
| `zerodds-amqp-bridge` | AMQP-1.0 gateway (per DDS-AMQP 1.0 spec) |
| `zerodds-amqp-endpoint` | AMQP DDS endpoint (the AMQP side, mirrors DDS) |
| `zerodds-grpc-bridge` | gRPC gateway |
| `zerodds-ros2-rmw` | ROS-2 RMW conversion helpers |
| `rmw-zerodds-shim` | extern-C ROS-2 RMW plugin |
| `zerodds-opcua-gateway` | OPC-UA pub/sub bridge |
| `zerodds-soap` | SOAP 1.2 / WSDL Web-Profile |
| `zerodds-xml-wire` | DDS-XML 1.0 wire format |

## CORBA + CCM

| Crate | Purpose |
|---|---|
| `dds-corba-{giop,iiop,ior,poa,cosnaming,ir,csiv2,codegen}` | CORBA 3.3 stack |
| `dds-corba-{cos-event,ccm,ccm-ejb,dnc,ccm-lib}` | CCM container + COS-Event-Service |
| `zerodds-corba-dds-bridge` | CORBA ↔ DDS bridge |

## DLRL

| Crate | Purpose |
|---|---|
| `zerodds-dlrl` | Data-Local-Reconstruction-Layer runtime |
| `zerodds-dlrl-codegen` | DLRL codegen |

## XTypes / IDL

| Crate | Purpose |
|---|---|
| `zerodds-idl` | OMG-IDL 4.2 parser, AST, builder, validator |
| `zerodds-idl-cpp` | C++ codegen backend |
| `zerodds-idl-csharp` | C# codegen backend |
| `zerodds-idl-java` | Java codegen backend |
| `zerodds-idl-ts` | TypeScript codegen backend (per DDS-TS 1.0 spec) |
| `zerodds-java-omgdds` | Pure-Java DDS-Java-PSM (`org.omg.dds.*`) — runtime + InProcessBus |
| `zerodds-c-api` | C-FFI: extern "C" runtime hub |

## CCM Containers + Misc

| Crate | Purpose |
|---|---|
| `zerodds-ccm` | OMG CCM 4.0 container |
| `zerodds-ami4ccm` | AMI4CCM async pattern |
| `zerodds-rtc` | Robotic Technology Component |
| `zerodds-time-service` | Time-Sync helper |

## Bindings

| Crate | Purpose |
|---|---|
| `zerodds-rs` | Rust idiomatic API (re-export) |
| `zerodds-sys` | C raw bindings |
| `zerodds-cpp` | C++17 RAII wrapper over zerodds.h |
| `zerodds-cs` | C# P/Invoke binding |
| `zerodds-py` | Python `pyo3` binding |
| `zerodds-ts-wasm` | TypeScript WASM XCDR codec |

## Tooling

| Crate | Purpose |
|---|---|
| `zerodds-lint` | Workspace-internal AST lints (no_panic, hot-path-realloc-free, …) |
| `zerodds-conformance` | Self-audit conformance test vectors |
| `zerodds-rt-linux` | Linux RT scheduler + CPU pinning (UNSAFE-FFI) |
| `zerodds-recorder` | `.zddsrec` recording format |
| `zerodds-monitor` | Live monitoring helper |
| `zerodds-http2`, `zerodds-hpack` | HTTP/2 + HPACK stacks (gRPC backbone) |
| `zerodds-sql-filter` | DDS SQL filter expression evaluator |

## Binary tools

| Tool | Purpose |
|---|---|
| `zerodds-admin` | Runtime introspection CLI |
| `zerodds-perf` | Perf bench + HW-info |
| `zerodds-idlc` | IDL compiler |
| `zerodds-xmlc` | DDS-XML compiler |
| `zerodds-traceability` | Wire-trace decoder |
| `zerodds-chaos` | Network-chaos proxy |
| `zerodds-replay` | `.zddsrec` playback |
| `zerodds-bench-suite::roundtrip-1us` | Sub-µs latency bench |
| `amqp-dds-endpoint` | AMQP demo endpoint |

## Dependency direction

```
foundation  ←  cdr  ←  types  ←  qos  ←  rtps  ←  discovery  ←  dcps
                                                                  │
                              transport-* ←─────────────────────  │
                              security-*  ←─────────────────────  │
                                                                  │
                                                bridges, bindings ←
```

No upward dependency. Bridges depend on dcps; dcps does not depend
on bridges. Bindings depend on dcps; dcps does not depend on
bindings.

## Crate-internal READMEs

Each crate ships a `README.md` with its own quick-start. Browse
them via `find crates -maxdepth 2 -name README.md`.
