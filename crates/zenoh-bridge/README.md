# `zerodds-zenoh-bridge`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-zenoh-bridge/badge.svg)](https://docs.rs/zerodds-zenoh-bridge)

Bidirectional bridge between ZeroDDS DCPS and Eclipse Zenoh: topic mapping,
QoS translation, and the `zerodds-zenoh-bridged` daemon. Wire-compatible
with Eclipse Zenoh's own DDS bridge
([`zenoh-plugin-dds`](https://github.com/eclipse-zenoh/zenoh-plugin-dds) /
`zenoh-bridge-dds`) — see [Wire compatibility](#wire-compatibility) below.
Safety classification: **STANDARD**.

## Architecture

```text
 DDS Writer  ─────────►  ZeroDDS DataReader (raw bytes, UserSample::Alive)
    │                       │
    │                       ▼ forward pump
    │                  + RTPS encapsulation header
    │                       │
    │                       ▼ zenoh Publisher::put / PublicationCache (TRANSIENT_LOCAL)
    │                  Zenoh KeyExpr  <scope>/<partition>/<topic>
    │                       │
    │  ◄────────────────────┤  Zenoh Subscriber / FetchingSubscriber (TRANSIENT_LOCAL)
    │                       │
    │                  reverse pump
    │                  − RTPS encapsulation header
    │                       ▼ DcpsRuntime::write_user_sample_borrowed / _keyed
    ▼
 DDS Reader
```

## What's inside

- **`mapping.rs`** (`std` feature, no `zenoh` dep) — `key_expr_for_topic`,
  `topic_allowed` (glob allow/deny), `congestion_control`,
  `is_transient_local`, and `encapsulation_header` /
  `decode_encapsulation_header` (the RTPS DDSI-RTPS 2.5 §10.5 framing a
  Zenoh payload carries). Pure Rust, unit-testable without a network.
- **`runtime.rs`** (feature `zenoh-runtime`) — `ZenohBridgeBuilder` /
  `ZenohBridge`: the engine that actually pumps samples between a
  `DcpsRuntime` and a Zenoh `Session`, in both directions, including the
  TRANSIENT_LOCAL late-joiner path and keyed-topic dispose.
- **`daemon/`** (feature `daemon`, default) — the `zerodds-zenoh-bridged`
  binary: CLI, layered YAML config, SIGHUP reload, systemd `Type=notify`,
  Prometheus `/metrics` + `/catalog` + `/healthz`, and auto-discovery of
  every DDS topic the runtime sees.

## Layer position

Layer 5 — bridges. Sits on [`zerodds-dcps`](../dcps) (the raw,
type-agnostic `DcpsRuntime` byte-sample channel — the same primitive
[`zerodds-routing-service`](../routing-service) and every other bridge
daemon use) and [`zerodds-qos`](../qos). The live runtime uses `zenoh = "1"`
+ `zenoh-ext` + `tokio`.

## Wire compatibility

Read against `zenoh-plugin-dds` / `zenoh-bridge-dds` tag `1.10.1` (see
`src/mapping.rs` and `src/runtime.rs` for the exact file/line citations):

- **Key expression**: `<scope>/<partition>/<topic>`, both optional; `scope`
  is empty by default, matching the plugin's own default (no `"dds/"`
  prefix). The historic ZeroDDS-only convention of a mandatory `"dds"`
  prefix is reachable by setting `zenoh.scope: "dds"` — same parameter, a
  non-default value.
- **Payload**: the full RTPS-encapsulated CDR blob (4-byte representation
  identifier + options, DDSI-RTPS 2.5 §10.5 Table 10.3), exactly the bytes
  the plugin puts — not the header-stripped body ZeroDDS' own internal
  channel carries.
- **`allow`/`deny`**: matched against the derived key expression, like the
  plugin, using this workspace's `*`/`?` glob convention instead of the
  plugin's regular expressions.
- **Congestion control**: `Block` for a `RELIABLE` DDS writer when
  `reliable_routes_blocking` (default on), `Drop` otherwise — the plugin's
  own rule, not a `Durability`-based guess.
- **TRANSIENT_LOCAL / late joiners**: a `PublicationCache` on the DDS→Zenoh
  side, a `FetchingSubscriber` querying `<key>/@dds_pub_cache/*` on the
  Zenoh→DDS side — the same `zenoh_ext` mechanism the plugin uses.
- **Dispose/unregister**: `zenoh-plugin-dds` does not forward DDS
  dispose/unregister in either direction (a documented gap in the plugin
  itself, not something this crate invented a workaround for out of
  nowhere). This project's rule against silently dropping a sample means
  `zerodds-zenoh-bridged` *does* forward it, as a Zenoh `delete` on
  `<key-expr>/<key-hash-hex>` — an extension of the plugin's key layout
  (a plain `zenoh-bridge-dds` peer that never sends such a delete is
  unaffected by it), not a replacement for it.

## Quickstart — pure-Rust mapping (no network)

```rust
use zerodds_zenoh_bridge::{key_expr_for_topic, topic_allowed};

assert_eq!(key_expr_for_topic(None, "", "Chatter").unwrap(), "Chatter");
assert_eq!(
    key_expr_for_topic(None, "robot1", "Sensor").unwrap(),
    "robot1/Sensor"
);
assert!(topic_allowed("robot1/Sensor", &["robot*/*".to_string()], &[]));
```

## Quickstart — the live engine (feature `zenoh-runtime`)

```toml
[dependencies]
zerodds-zenoh-bridge = { version = "1.0.0-rc.7", features = ["zenoh-runtime"] }
```

```rust,ignore
use std::sync::Arc;
use zerodds_zenoh_bridge::runtime::{Direction, TopicSpec, ZenohBridgeBuilder};

let dcps = Arc::new(zerodds_dcps::runtime::DcpsRuntime::start(
    0, prefix, runtime_config,
)?);
let bridge = ZenohBridgeBuilder::new()
    .with_dcps_runtime(dcps)
    .build()
    .await?;
bridge
    .add_topic(TopicSpec {
        dds_topic: "Chatter".into(),
        dds_type: "std_msgs::String".into(),
        direction: Direction::Both,
        ..TopicSpec::default()
    })
    .await?;
```

## Running the daemon

Build it (the `daemon` feature is in `default`):

```bash
cargo build --release -p zerodds-zenoh-bridge --bin zerodds-zenoh-bridged
```

Minimal config (`zenoh-bridged.yaml`, see
`packaging/linux/configs/zenoh-bridged.yaml.example` for the full annotated
version and `man/man5/zerodds-zenoh-bridged.yaml.5`):

```yaml
domain: 0
zenoh:
  mode: peer
discovery:
  auto_bridge: true
metrics:
  enabled: true
  listen: "127.0.0.1:9097"
```

```bash
./target/release/zerodds-zenoh-bridged --check-config --config zenoh-bridged.yaml
./target/release/zerodds-zenoh-bridged --config zenoh-bridged.yaml
```

### Against `zenohd` + `z_sub` / `z_put`

Run on one Linux host (or three terminals on one host). `zenohd` and the
`z_sub`/`z_put` example tools come from the `zenoh` project's own releases
(`apt install zenoh` from Zenoh's package repo, or `cargo install --locked
zenohd` and build the `zenoh` crate's examples).

```bash
# Terminal 1 — a Zenoh router.
zenohd

# Terminal 2 — the bridge, as a client of the router, bridging domain 0
# with auto-discovery on (so it picks up whatever topic a DDS writer
# publishes without pre-listing it).
./target/release/zerodds-zenoh-bridged \
    --mode client --connect tcp/127.0.0.1:7447 --domain 0

# Terminal 3 — watch every key the bridge routes.
z_sub -k '**'

# Terminal 4 — any real DDS participant on domain 0 that publishes a topic
# (e.g. `zerodds-record --domain 0 --topic Chatter` from this workspace, or
# any DDS vendor's own publisher). z_sub in terminal 3 shows the routed
# samples arrive as the full RTPS-encapsulated CDR blob (see
# "Wire compatibility" above) — e.g. `00 01 00 00 ...` for a
# little-endian XCDR1 body.
```

A `z_put` into a key the bridge is subscribed to (Zenoh→DDS) needs a
correctly RTPS-encapsulated CDR payload to be accepted — an arbitrary
`z_put -k Chatter -v hello` is refused (logged + counted in
`zerodds_zenoh_zenoh_to_dds_total` / the "no valid RTPS encapsulation
header" log line), not silently forwarded as garbage into DDS. For a
meaningful Zenoh→DDS demo use a real DDS-aware peer, as below.

### Against `zenoh-bridge-dds` with a Cyclone DDS peer

This is the actual interop check: a `zenoh-bridge-dds` (built on
CycloneDDS) bridges DDS domain *N* on one host/process into the same Zenoh
network `zerodds-zenoh-bridged` bridges DDS domain *N* from — via ZeroDDS —
into. With `discovery.auto_bridge: true` on both sides (the default; a
`zenoh-bridge-dds` process defaults to open discovery too) neither side
needs to know the other's topic names in advance.

```bash
# Terminal 1 — zenoh-bridge-dds, itself the DDS<->Zenoh bridge for a
# CycloneDDS-backed participant on domain 0, listening for peers.
zenoh-bridge-dds --mode peer --listen tcp/0.0.0.0:7447 --domain 0

# Terminal 2 — zerodds-zenoh-bridged, connecting to it, bridging the SAME
# DDS domain on the ZeroDDS side.
./target/release/zerodds-zenoh-bridged \
    --mode peer --connect tcp/127.0.0.1:7447 --domain 0

# Terminal 3 — a genuine CycloneDDS participant (any CycloneDDS tool or
# your own idlc-generated pub/sub) publishing on domain 0. CycloneDDS's own
# `ddsperf` is convenient for a connectivity check (adjust -i/-D to your
# CycloneDDS build; the exact topic name it uses is whatever your
# CycloneDDS version publishes — auto_bridge above picks it up without
# needing to name it):
ddsperf -i 0 pub -D 5

# Terminal 4 — a ZeroDDS reader on the SAME topic name, domain 0, verifying
# the sample crossed CycloneDDS -> zenoh-bridge-dds -> Zenoh ->
# zerodds-zenoh-bridged -> ZeroDDS.
```

Check `http://127.0.0.1:9097/catalog` (`zerodds-zenoh-bridged`'s admin
endpoint, if `metrics.enabled: true`) and `zenoh-bridge-dds`'s own admin
space (`z_get -s '@/*/dds/**'` against the same Zenoh network) to see what
each side actually bridged.

## Feature flags

| Feature | Default | Purpose |
|---------|---------|---------|
| `std` | ✅ | The pure-Rust mapping logic (`mapping.rs`), no `zenoh` dep. |
| `zenoh-runtime` | via `daemon` | The live engine: `zenoh` + `zenoh-ext` + `tokio`. |
| `daemon` | ✅ | The `zerodds-zenoh-bridged` binary (implies `zenoh-runtime`). |

`--no-default-features --features std` builds and tests the pure mapping
logic alone, with no external dependency.

## Stability

`1.0.0-rc.7`. The mapping logic and wire-compatibility conventions are
RC-stable; the live-engine and daemon APIs follow `zenoh = "1.x"` and may
change with a Zenoh major bump.

## Tests

```bash
cargo test -p zerodds-zenoh-bridge
cargo test -p zerodds-zenoh-bridge --all-features
```

Unit tests for the mapping/QoS/wire-framing logic and the daemon config
parser, plus in-process end-to-end tests (a real `DcpsRuntime` writer/reader
against the daemon's engine and an in-process Zenoh session) covering
plain forward/reverse pumping, a keyed topic with dispose, a
TRANSIENT_LOCAL late joiner, and a 1 MiB payload. See
`tests/daemon_e2e.rs`.

## License

Apache-2.0. See [LICENSE](../../LICENSE).

## See also

- [Eclipse Zenoh](https://zenoh.io/) — upstream project.
- [`zenoh-plugin-dds`](https://github.com/eclipse-zenoh/zenoh-plugin-dds) —
  the wire-compatibility reference this crate was read against.
