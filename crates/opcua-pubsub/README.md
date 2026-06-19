# zerodds-opcua-pubsub

OPC-UA Pub/Sub **Part 14** (UADP) — native pure-Rust wire stack for
ZeroDDS. `no_std + alloc`.

This crate adds the OPC Foundation **Part 14** publish/subscribe transport
on top of [`zerodds-opcua-gateway`](https://docs.rs/zerodds-opcua-gateway)
(the OMG DDS-OPCUA 1.0 type-system + AddressSpace mapping). The two are
distinct spec families: the gateway bridges OPC-UA into the DDS global data
space; Part 14 is OPC-UA's own pub/sub wire protocol. This crate is an
additive profile, not a change to the gateway.

## Layers

1. **`binary`** — OPC-UA Part 6 binary encoding (`UaEncode` / `UaDecode`
   for `NodeId`, `ExpandedNodeId`, `Guid`, `QualifiedName`,
   `LocalizedText`, `ExtensionObject`, `Variant`, `DataValue`). The
   foundation for UADP DataSetMessages — and natively usable by the
   Client/Server gateway.
2. UADP `NetworkMessage` / `DataSetMessage` framing.
3. PubSub configuration model + `DataSetWriter` / `DataSetReader`.
4. UADP discovery (`DataSetMetaData`).
5. PubSub security (`SecurityGroup` + Security Key Service).
6. Transport carriers (UDP multicast, Ethernet, MQTT/AMQP).
7. DataSet ↔ DDS-topic bridge + daemon.

## License

Apache-2.0
