# `zerodds-opcua-bridge`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-opcua-bridge/badge.svg)](https://docs.rs/zerodds-opcua-bridge)

The OMG **DDS-OPCUA Gateway 1.0** (formal/2020-01-01) as a running daemon,
`zerodds-opcua-bridged`, on `zerodds-dcps`. The four library crates
(`zerodds-opcua-gateway`, `-uacp`, `-server`, `-pubsub`) implement the type
system, the `opc.tcp` framing/SecureChannel, the Part 4 Client/Server
service set and the Part 14 (UADP) PubSub wire stack; this crate is the
process that runs them: an `opc.tcp` server exposing DDS topics, an
OPC-UA client bridging foreign servers into DDS, and Part-14 PubSub
publishers/subscribers over UDP multicast — configured from one YAML file.

Part of [**ZeroDDS**](../../README.md). Safety classification: **STANDARD**.

## What's inside

- **`dynamic_type`** — `zerodds_types::dynamic::DynamicType` →
  `zerodds_opcua_gateway::dds_to_ua::walker::DdsType`.
- **`dynamic_value`** — `DynamicData` ↔ a flat, path-named `Variant` list
  (`flatten`/`unflatten`), the shape both the AddressSpace's leaves and a
  Part-14 DataSet's fields need.
- **`idl`** — compiles an operator-supplied `.idl` file and builds the
  `SerializedTypeSupport` a topic needs (Spec item: "types via TypeObject/
  TypeLookup").
- **`dds_bridge`** — the DDS-side port: participant/topic/reader/writer
  registration against a real `zerodds-dcps::runtime::DcpsRuntime`.
- **`address_space_build`** — Gateway 1.0 §9.3 AddressSpace materialization.
- **`ua_client`** — Gateway 1.0 §8's client role against foreign servers.
- **`pubsub_glue`** — the Part 14 UADP Publisher/Subscriber wiring.
- **`daemon`** — the process itself: CLI, YAML config, SIGHUP/SIGTERM,
  systemd `Type=notify`, Prometheus metrics.

See `docs/specs/zerodds-opcua-bridge-1.0.md` for the full contract.

## Quickstart

```bash
cargo build -p zerodds-opcua-bridge --bin zerodds-opcua-bridged --features crypto
```

```yaml
# opcua-bridged.yaml
domain: 0
opcua:
  idl: ["robotics.idl"]
  gateway_config: "gateway.xml"   # Spec §10 XML
  server:
    bind: "0.0.0.0:4840"
```

```bash
zerodds-opcua-bridged --config opcua-bridged.yaml
```

## Measuring against open62541 1.5.4

The four library crates are measured against open62541 1.5.4 directly
(`crates/opcua-server/proofs/open62541.sh`,
`crates/opcua-pubsub/proofs/open62541_uadp.sh`), built on the Linux
measurement host under `/root/o62-ex`/`/root/o62-ex2`
(`crates/opcua-pubsub/proofs/build-open62541.sh` — see
`docs/interop-harnesses.md`). Those measure the library; the commands below
measure the **daemon process** on top of it, which is not yet part of an
automated proof script in this pass (§10 of the spec file is an open item).

```bash
# 1. Start the daemon (SecurityMode None; add --features crypto + a
#    security_policies entry for a secured measurement).
zerodds-opcua-bridged --config opcua-bridged.yaml &

# 2. From an open62541 build (client_connect / a custom client using
#    UA_Client_connect + UA_Client_readValueAttribute), GetEndpoints/
#    Browse/Read/Subscribe against the daemon's opc.tcp endpoint:
/root/o62-ex/bin/examples/client_connect opc.tcp://127.0.0.1:4840

# 3. Or any GetEndpoints/Browse/Read/Subscribe-capable generic OPC-UA
#    client (e.g. UaExpert) pointed at the same endpoint — the daemon
#    speaks plain Part 4/6, nothing vendor-specific.
```

For the Part-14 direction, `crates/opcua-pubsub/proofs/open62541_uadp.sh`'s
own multicast group/PublisherId/WriterGroupId/DataSetWriterId
(`tutorial_pubsub_publish`/`tutorial_pubsub_subscribe`) can be mirrored in
`opcua.pubsub.connections[]` to repeat that measurement against the daemon
instead of the bare library.

## Feature flags

- `std` (default) — the crate's non-`daemon` conversion modules.
- `daemon` (default) — everything: the reflective type layer, the DDS
  port, the AddressSpace/PubSub glue, the process itself.
- `crypto` — secured SecureChannel SecurityPolicies
  (`basic256sha256`/`aes128_sha256_rsaoaep`/`aes256_sha256_rsapss`); pulls
  `rsa`/`pem`/`rand`.
- `ring-backend` (default) / `aws-lc` — the crypto backend
  `zerodds-bridge-security`'s ACL layer pulls in.
- `rt` — `zerodds-dcps/rt` real-time scheduling passthrough.

## Tests

```bash
cargo test -p zerodds-opcua-bridge --lib             # 51 unit tests
cargo test -p zerodds-opcua-bridge --test daemon_e2e # 3 in-process e2e tests
cargo test -p zerodds-opcua-bridge --test example_yaml_loadback
```

The Part-14 e2e test (`dds_publish_reaches_an_in_tree_pubsub_subscriber`)
uses real UDP multicast; macOS loopback multicast is measured to be
unreliable on this host family — the test reports plainly
instead of asserting on a race if nothing arrives within its deadline.
Measure on Linux for a firm result.

## License

Apache-2.0
