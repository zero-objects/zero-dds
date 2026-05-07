# ZeroDDS User Handbook

A consolidated guide for application developers who want to ship
a DDS-based system on ZeroDDS. Skim this once; jump to the trail
stations (`01-getting-started/` … `07-migration/`) when you need
the long form.

---

## 1. What is ZeroDDS

ZeroDDS is a pure-Rust implementation of the OMG Data
Distribution Service (DDS 1.4) and the wire-level RTPS 2.5
protocol, with full support for DDS-XTypes 1.3 and DDS-Security
1.2. It is byte-compatible with Cyclone DDS, eProsima Fast-DDS,
RTI Connext, and OpenDDS, and ships first-class language
bindings for C, C++, C#, Java, Python, and TypeScript, plus a
ROS-2 RMW shim. The codebase is Apache-2.0, single-binary on
Linux / macOS / Windows, with no per-seat licence and no C/C++
in the hot path.

You write IDL → generate type-safe code → publish and subscribe
on a topic → ZeroDDS handles discovery, reliability,
fragmentation, and security on the wire.

---

## 2. Install

### Linux (Debian / Ubuntu)

```bash
curl -fsSL https://apt.zerodds.org/key.gpg | sudo apt-key add -
echo "deb https://apt.zerodds.org/ stable main" \
    | sudo tee /etc/apt/sources.list.d/zerodds.list
sudo apt update
sudo apt install zerodds-tools libzerodds-dev
```

### Linux (RHEL / Fedora)

```bash
sudo dnf install zerodds-tools zerodds-devel
```

### macOS

```bash
brew tap zero-objects/zerodds
brew install zerodds
```

### Windows

Download the latest `.msi` from <https://github.com/zero-objects/zero-dds/releases>
and run the installer. The MSI registers `zerodds-idlc`,
`zerodds-admin`, and `zerodds-perf` on `PATH`.

### Cargo (any platform, Rust 1.85+)

```bash
cargo add zerodds                 # high-level DCPS API
cargo install zerodds-idlc        # IDL → Rust codegen
```

### Docker

```bash
docker pull ghcr.io/zero-objects/zero-dds:latest
docker run --rm -it --network host ghcr.io/zero-objects/zero-dds zerodds-admin --help
```

`--network host` is the simplest setup — DDS uses multicast for
discovery. For bridged networks, use the unicast static
peer-list (see `03-configuration/transport.md`).

Verify with:

```bash
zerodds-idlc --version
zerodds-admin --topics
```

---

## 3. First Pub/Sub

Define a topic in IDL — `topics/sensor.idl`:

```idl
@topic
@final
struct Sensor {
    @key long sensor_id;
    double value;
    long long timestamp_ns;
};
```

Generate Rust bindings:

```bash
zerodds-idlc --lang rust --out src/topics topics/sensor.idl
```

Publisher (`src/bin/publisher.rs`):

```rust
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use zerodds::prelude::*;

mod topics;
use topics::Sensor;

fn main() -> anyhow::Result<()> {
    let dp = DomainParticipantFactory::get_instance()
        .create_participant(0)?;
    let topic  = dp.create_topic::<Sensor>("Temperature")?;
    let pub_   = dp.create_publisher()?;
    let writer = pub_.create_datawriter(&topic)?;

    for i in 0..100 {
        writer.write(&Sensor {
            sensor_id: 1,
            value: 20.0 + (i as f64) * 0.1,
            timestamp_ns: SystemTime::now()
                .duration_since(UNIX_EPOCH)?.as_nanos() as i64,
        })?;
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}
```

Subscriber (`src/bin/subscriber.rs`):

```rust
use zerodds::prelude::*;

mod topics;
use topics::Sensor;

fn main() -> anyhow::Result<()> {
    let dp = DomainParticipantFactory::get_instance()
        .create_participant(0)?;
    let topic  = dp.create_topic::<Sensor>("Temperature")?;
    let sub    = dp.create_subscriber()?;
    let reader = sub.create_datareader(&topic)?;

    loop {
        for sample in reader.take()? {
            println!(
                "id={} value={:.2}",
                sample.data.sensor_id, sample.data.value,
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}
```

Run them in two terminals:

```
$ cargo run --bin subscriber &
$ cargo run --bin publisher
id=1 value=20.00
id=1 value=20.10
id=1 value=20.20
…
```

Both processes discover each other via SPDP multicast on
domain 0. No broker needed.

---

## 4. QoS cheat-sheet

DDS QoS knobs are layered: defaults are sane, you only override
what your use case demands. The ten you need most:

| Policy | Default | Tune when |
|---|---|---|
| `Reliability` | `BestEffort` | You need lossless delivery — set `Reliable`. |
| `Durability` | `Volatile` | Late-joining subscribers must see history — set `TransientLocal`. |
| `History` | `KeepLast(1)` | Subscribers must see N back-samples — set `KeepLast(N)`. |
| `Deadline` | `infinite` | Detect stale writers — set a max inter-sample gap. |
| `Liveliness` | `Automatic, infinite` | Detect dead writers fast — `ManualByTopic` + finite lease. |
| `Ownership` | `Shared` | Active/standby pattern — set `Exclusive` + strength. |
| `Partition` | `[]` | Sub-topic isolation per tenant — set partition expression. |
| `ResourceLimits` | unbounded | Bound RAM in production — set `max_samples` + `max_instances`. |
| `LatencyBudget` | `0` | Hint to the transport that batching is OK. |
| `TransportPriority` | `0` | Multi-topic prioritisation on a saturated link. |

The full reference, including the assignability matrix and the
defaults table, lives in `03-configuration/qos-policies.md`.

---

## 5. Bridge setup

ZeroDDS ships protocol bridges that bring DDS topics onto
non-DDS transports. Pick by use-case.

| Use case | Bridge | Crate |
|---|---|---|
| Hospital / IIoT MQTT broker fleet | DDS ↔ MQTT 5.0 | `mqtt-bridge` |
| Microservice mesh on AMQP 1.0 (RabbitMQ, Azure Service Bus, ActiveMQ) | DDS ↔ AMQP 1.0 | `amqp-bridge` (+ `amqp-endpoint` for direct broker-less peers) |
| Browser / mobile clients over a WebSocket | DDS ↔ WebSocket binary frames | `ws-bridge` |
| RESTful enterprise integration | DDS ↔ HTTP CoAP-style request/response | `coap-bridge` |
| gRPC/Protobuf microservices | DDS ↔ gRPC unary + streaming | `grpc-bridge` |
| ROS-2 nodes | RMW shim — `rmw_zerodds` | `rmw-zerodds` |

Each bridge runs as its own daemon (`zerodds-mqtt-bridge`,
`zerodds-amqp-bridge`, …) configured by a YAML file. Wire-level
behaviour is documented in the published vendor specs (see
`specs/dds-amqp-1.0/main.pdf` for the AMQP wire-mapping).

---

## 6. Security setup

ZeroDDS implements DDS-Security 1.2 with the four mandatory
plugins: Authentication (PKI), AccessControl (permissions XML),
Cryptographic (AES-GCM-256), and Logging.

Minimal config — `zerodds.toml`:

```toml
[security]
identity_ca       = "ca/ca.pem"
identity_cert     = "ca/node.pem"
identity_key      = "ca/node.key"
permissions_ca    = "ca/perms-ca.pem"
governance        = "policies/governance.p7s"
permissions       = "policies/permissions.p7s"

[transport]
mode  = "tls"
cert  = "ca/node.pem"
key   = "ca/node.key"

[acl]
default_policy = "deny"
allow = [
    { topic = "Telemetry/*",   role = "publisher" },
    { topic = "Commands/*",    role = "subscriber" },
]
```

Bearer-token auth (for dynamic peers without certificates) is
opt-in via `[auth] bearer_jwks_url = "..."`. The full security
walkthrough lives in `03-configuration/security.md`.

---

## 7. Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| Pub and Sub never discover each other | Multicast disabled (cloud / VPC) | Use unicast static peer-list — see `03-configuration/transport.md` |
| `History` cache evicting samples | Reliable + KeepAll + slow Sub | Raise `ResourceLimits.max_samples` or switch to `KeepLast(N)` |
| Sub gets old samples on join | Default `Volatile` durability | Set `Durability=TransientLocal` on both sides |
| `liveliness_lost_count > 0` | Writer thread starved | Check scheduler / RT profile — see `06-operations/troubleshooting.md` |
| Cross-vendor: my Cyclone Sub sees no data | Wrong domain ID | Match `CYCLONEDDS_URI` `Domain Id` and ZeroDDS `domain_id` |
| Security handshake fails | Wrong CA chain | Verify with `openssl verify -CAfile ca.pem node.pem` |
| p99 latency spikes ~1 s on RT | RCU callbacks on RT core | Enable `nohz_full` + `rcu_nocbs` on the RT cores |

A deeper checklist with `zerodds-admin` invocations is in
`06-operations/troubleshooting.md`.

---

## 8. Spec mapping

Which OMG specification covers which ZeroDDS feature.

| OMG spec | What it covers | ZeroDDS implementation |
|---|---|---|
| DDS 1.4 (formal/2015-04-10) | DCPS API: Participant / Pub / Sub / Topic / DataWriter / DataReader / QoS | `crates/dcps`, `crates/dcps-async` |
| DDSI-RTPS 2.5 | Wire format, discovery, reliability, fragmentation | `crates/rtps`, `crates/discovery` |
| DDS-Security 1.2 | Authentication, AccessControl, Cryptographic, Logging plugins | `crates/security-pki`, `crates/security-crypto` |
| DDS-XTypes 1.3 | Type system, TypeObject, Assignability, IDL semantics | `crates/xtypes`, `crates/idl` |
| DDS-RPC 1.0 | Request/response patterns over DCPS | `crates/dds-rpc` |
| DDS-XML 1.0 | XML config + QoS profiles | `crates/dds-xml` |
| DDS-XRCE 1.0 | Constrained-device gateway protocol | `crates/dds-xrce` |
| DDS-AMQP 1.0 (vendor spec) | DDS ↔ AMQP 1.0 wire-mapping | `crates/amqp-bridge`, `crates/amqp-endpoint` |
| DDS-TS 1.0 (vendor spec) | TypeScript binding | `crates/zerodds-ts` |

For per-spec audit status, see `docs/spec-coverage/` (internal
repo only — published audit summaries land in the trail
station READMEs).

---

## Where to next

- Production-ready deployment: `operator-guide/HANDBOOK.md`.
- Contributing to the codebase: `developer-guide/HANDBOOK.md`.
- Trail in full: `01-getting-started/` through `07-migration/`.
