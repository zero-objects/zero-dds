<!--
SPDX-License-Identifier: Apache-2.0
Copyright 2026 ZeroDDS Contributors
-->

# zerodds-routing-service

A standalone **DDS routing service** for [ZeroDDS](../../README.md): it forwards
samples between DDS domains, topics, QoS profiles and partitions **within** the
DDS bus. It is the in-DDS routing counterpart to the protocol bridges (which
cross protocol boundaries), and the ZeroDDS equivalent of the *RTI Routing
Service*.

Safety classification: **COMFORT**.

## What it does

A **route** couples an input endpoint (a reader on a domain/topic) to an output
endpoint (a writer on a — possibly different — domain/topic):

| Capability | Description |
|---|---|
| **Topic renaming** | input topic `A` → output topic `B` |
| **Domain bridging** | read on domain 0, write on domain 1, in one process |
| **QoS mapping** | per-endpoint reliability / durability / ownership / partition / data-representation |
| **Type-agnostic byte forwarding** | the CDR body is forwarded verbatim — representation-faithful (XCDR1/XCDR2 encapsulation preserved), no type recompilation, cross-vendor |
| **Keyed-instance lifecycle** | dispose / unregister events are forwarded for keyed topics (Spec §9.6.3.9); with `type_file` on the output endpoint the marker carries the `serializedKey` Cyclone and OpenDDS need (see below) |
| **Loop guard** | input and output participants on a shared domain are made mutually invisible, so no router output is re-ingested by a router input — preventing router-internal forwarding loops (bidirectional bridges, multi-route cycles) while application writers stay visible |
| **Content filtering** | a DDS SQL filter (`temp > 50 AND zone = 'A'`) evaluated per sample against the decoded body |
| **Field transformation** | rename / constant-set / drop members, mapping an input shape to an output shape |

## Surfaces

* **Library** — `Router::start` / `Router::start_with_types`.
* **Config** — JSON (`RouterConfig::from_json`) or XML (`RouterConfig::from_xml`,
  native ZeroDDS schema + an RTI-Routing-Service-compatible subset).
* **Daemon** — the `zerodds-router` binary.
* **Metrics** — per-route counters in the `zerodds-monitor` registry (rendered by
  the existing Prometheus exporter).

## Daemon

```console
# Validate a config without starting any DDS state.
zerodds-router validate --routes router.json

# Run the router (byte forwarding + loop guard + QoS mapping).
zerodds-router run --routes router.json

# Run with type shapes (needed for content filtering / field transformation).
zerodds-router run --routes router.json --types shapes.json
```

`SIGINT` / `Ctrl-C` shuts the router down cleanly.

## Config — JSON

```json
{
  "name": "edge-to-core",
  "routes": [
    {
      "name": "telemetry",
      "input":  { "domain": 0, "topic": "Telemetry", "type_name": "sensors::Reading" },
      "output": { "domain": 1, "topic": "Telemetry", "type_name": "sensors::Reading",
                  "qos": { "durability": "transient_local" } },
      "filter": { "expression": "temp > 50" }
    }
  ]
}
```

Routes without a `filter`/`transform` forward bytes verbatim and need no type
information.

A key the model does not read is refused; a key that begins with `_` is a
comment (`"_comment": "…"`). Left out, an endpoint's QoS is RELIABLE,
VOLATILE and SHARED.

### Validation

A configuration is refused, with the reason, when

* two routes have the same name;
* the input and the output of a route disagree on `keyed`;
* a route reads and writes the same domain, topic and partition with
  `"loop_guard": false`;
* `type_file` stands on the input endpoint, or on a route that is not keyed;
* a relay route (`"relay": true`, forwarding unchanged on the writer's behalf,
  DDS-Security 1.2 §7.1.1.4) has another domain, topic or type on its output,
  a filter or a transform;
* an `access_control` list is present and an endpoint matches none of its
  `allow` rules. A rule names a tenant (left out: any), a domain and topic
  globs, where `*` matches any run of characters and `?` one.

A keyed route forwards a dispose or unregister by its key hash. Without a type
the output marker carries `PID_KEY_HASH` alone, and Cyclone DDS 11.0.1 and
OpenDDS 3.35 do not act on it (measured: the instances stay `ALIVE`).
`"type_file"` on the **output** endpoint names a serialized COMPLETE TypeObject
of the output type (the format `zerodds-zenoh-bridged` and
`zerodds-amqp-bridged` read); the output writer then keys every forwarded
sample, records each instance's key, and a forwarded dispose carries the
`serializedKey`. The type is not announced, peers still match by type name.

```json
"output": { "domain": 1, "topic": "Orders", "type_name": "shop::Order",
            "keyed": true, "type_file": "/etc/zerodds/shop-order.typeobject" }
``` A route **with** a filter or transform needs the input/output type
**shape** (the flat struct member layout) supplied via `--types`:

```json
[
  { "name": "sensors::Reading",
    "members": [ { "name": "temp", "kind": "i32" }, { "name": "zone", "kind": "string" } ] }
]
```

## Config — XML

Both the native `<zerodds_routing>` schema and an RTI-Routing-Service-compatible
subset (`<dds><routing_service>…<domain_route><session><topic_route>`) are
accepted by `RouterConfig::from_xml`, easing migration from existing RTI
deployments.

## Library

```rust
use zerodds_routing_service::{Router, RouterConfig};

let cfg = RouterConfig::from_json(json)?;
let router = Router::start(&cfg)?;          // byte forwarding + loop guard
// … router runs on background pump threads until dropped …
let m = router.route_metrics("telemetry").unwrap();
println!("forwarded {} samples", m.forwarded);
```

A route with `"preserve_source_timestamp": true` writes each sample with the
source timestamp it came in with instead of the router's clock.
`"max_samples_per_second": n` forwards at most a burst of `n` samples, then
`n` per second, and counts the rest as dropped.

## Limitations

* **Filter / transform** operate on flat `@final` / `@appendable` structs of
  scalar and string members (the common routing case), read in the format and
  the byte order the sample's encapsulation header names. Nested structs,
  sequences, unions and `@mutable` member headers are refused per sample;
  byte pass-through routing has no such restriction.
* **Loop guard** covers router-*internal* loops (within one process). Loops
  spanning two separate `zerodds-router` processes need a discovery
  discriminator (a router tag in `user_data`), which the DCPS builtin
  publication reader does not currently surface — a documented follow-up.

## Tests

`cargo test -p zerodds-routing-service` runs the unit tests (configuration and
its validation, the XML forms, the codec against the vendors' octets of the
interop corpus, the SQL filter, the transform, the rate limit) and the targets
over real participants on leased domains:

* `encapsulation_forwarding`, `byte_order_processing` — a forwarded sample keeps
  the representation identifier it came in; filter and transform read the body
  in the format and byte order of its header.
* `loop_guard` — a bidirectional bridge does not feed back on itself.
* `relay_route` — a relay route forwards on behalf of the writer.
* `preserve_timestamp` — the source timestamp survives the route.
* `key_holder_reaches_the_output_datagram` — a forwarded dispose carries its key.

The daemon between writer and reader processes, with topic renaming and a
content filter, is the scenario `services::routing_forwards_and_filters` of the
integration pillar (`tests/integration`). A routed dispose read by Cyclone DDS
and Fast DDS (`dispose_reaches_a_foreign_reader`, ignored here) runs in the
weekly job `vendor-live-dispose-services`.

## License

Apache-2.0. Part of [ZeroDDS](../../README.md).
