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
| **Keyed-instance lifecycle** | dispose / unregister events are forwarded for keyed topics (Spec §9.6.3.9) |
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
zerodds-router validate --config router.json

# Run the router (byte forwarding + loop guard + QoS mapping).
zerodds-router run --config router.json

# Run with type shapes (needed for content filtering / field transformation).
zerodds-router run --config router.json --types shapes.json
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
information. A route **with** a filter or transform needs the input/output type
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

## Limitations

* **`preserve_source_timestamp`** is parsed but rejected at route build: the
  runtime user byte-path does not surface a sample's source timestamp, so the
  router cannot reproduce it. Wiring `source_timestamp` through the user byte
  path is the prerequisite follow-up — the option fails loudly rather than being
  silently ignored.
* **Filter / transform** operate on flat `@final` / `@appendable` structs of
  scalar and string members (the common routing case), decoded little-endian.
  Nested structs, sequences, unions and `@mutable` member headers are a
  documented extension; byte pass-through routing has no such restriction.
* **Loop guard** covers router-*internal* loops (within one process). Loops
  spanning two separate `zerodds-router` processes need a discovery
  discriminator (a router tag in `user_data`), which the DCPS builtin
  publication reader does not currently surface — a documented follow-up.

## Tests

`cargo test -p zerodds-routing-service --all-features` runs the unit suite
(config, XML, byte-exact XCDR1/XCDR2/appendable-DHEADER codec, SQL bridge,
transform planning) plus the end-to-end suite over real DDS participants:

* `forward_cross_domain` — app writer → router → reader across a domain bridge
  with topic rename; bodies arrive verbatim.
* `loop_guard` — a bidirectional bridge does not feed back on itself.
* `filter_transform` — a SQL content filter drops non-matching samples on the
  wire.
* `keyed_lifecycle` — a dispose event forwards across domains and flips the
  downstream instance state to `NOT_ALIVE_DISPOSED`.

## License

Apache-2.0. Part of [ZeroDDS](../../README.md).
