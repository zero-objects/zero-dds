# First publisher and subscriber

This chapter wires a publisher and subscriber together using the
Rust API directly. For non-Rust languages, jump to
[Integration](../README.md) and pick yours.

## Setup

In a fresh directory:

```bash
cargo new --bin hello-zerodds
cd hello-zerodds
```

Edit `Cargo.toml`:

```toml
[dependencies]
zerodds-dcps = { git = "https://github.com/zero-objects/zero-dds.git" }
zerodds-rtps = { git = "https://github.com/zero-objects/zero-dds.git" }
zerodds-qos  = { git = "https://github.com/zero-objects/zero-dds.git" }
zerodds-types = { git = "https://github.com/zero-objects/zero-dds.git" }
```

(For now we depend on the lower-level crates directly; the
high-level `zerodds` re-export crate is in progress.)

## Publisher

`src/bin/pub.rs`:

```rust
use std::thread::sleep;
use std::time::Duration;

use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserWriterConfig};
use zerodds_qos::{
    DeadlineQosPolicy, DurabilityKind, LifespanQosPolicy, LivelinessQosPolicy, OwnershipKind,
};
use zerodds_rtps::wire_types::GuidPrefix;
use zerodds_types::{PrimitiveKind, TypeIdentifier};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let domain_id = 1;
    let prefix = GuidPrefix::from_bytes([0x01; 12]);
    let rt = DcpsRuntime::start(domain_id, prefix, RuntimeConfig::default())?;

    let writer_eid = rt.register_user_writer(UserWriterConfig {
        topic_name: "HelloTopic".into(),
        type_name: "RawBytes".into(),
        reliable: true,
        durability: DurabilityKind::Volatile,
        deadline: DeadlineQosPolicy::default(),
        lifespan: LifespanQosPolicy::default(),
        liveliness: LivelinessQosPolicy::default(),
        ownership: OwnershipKind::Shared,
        ownership_strength: 0,
        partition: vec![],
        user_data: vec![],
        topic_data: vec![],
        group_data: vec![],
        type_identifier: TypeIdentifier::Primitive(PrimitiveKind::UInt8),
        data_representation_offer: None,
    })?;

    for i in 0u32..10 {
        let payload = format!("hello {i}").into_bytes();
        rt.write_user_sample(writer_eid, payload)?;
        println!("pub: sent {i}");
        sleep(Duration::from_millis(500));
    }
    rt.shutdown();
    Ok(())
}
```

## Subscriber

`src/bin/sub.rs`:

```rust
use std::time::Duration;

use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample};
use zerodds_qos::{DeadlineQosPolicy, DurabilityKind, LivelinessQosPolicy, OwnershipKind};
use zerodds_rtps::wire_types::GuidPrefix;
use zerodds_types::{PrimitiveKind, TypeIdentifier, qos::TypeConsistencyEnforcement};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let domain_id = 1;
    let prefix = GuidPrefix::from_bytes([0x02; 12]);
    let rt = DcpsRuntime::start(domain_id, prefix, RuntimeConfig::default())?;

    let (_eid, rx) = rt.register_user_reader(UserReaderConfig {
        topic_name: "HelloTopic".into(),
        type_name: "RawBytes".into(),
        reliable: true,
        durability: DurabilityKind::Volatile,
        deadline: DeadlineQosPolicy::default(),
        liveliness: LivelinessQosPolicy::default(),
        ownership: OwnershipKind::Shared,
        partition: vec![],
        user_data: vec![],
        topic_data: vec![],
        group_data: vec![],
        type_identifier: TypeIdentifier::Primitive(PrimitiveKind::UInt8),
        type_consistency: TypeConsistencyEnforcement::default(),
        data_representation_offer: None,
    })?;

    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    while std::time::Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(UserSample::Alive { payload, .. }) => {
                println!("sub: got {}", String::from_utf8_lossy(payload.as_ref()));
            }

            Ok(UserSample::Lifecycle { key_hash, kind }) => {
                println!("lifecycle: {:?} {:?}", kind, key_hash);
            }

            Err(_) => {}
        }
    }
    rt.shutdown();
    Ok(())
}
```

## Run it

In two terminals:

```bash
# Terminal 1
cargo run --bin sub

# Terminal 2 (start ~1 second later so subscriber is listening)
cargo run --bin pub
```

Expected output:

```text
sub: got hello 0
sub: got hello 1
sub: got hello 2
...
```

## Cross-host

The same code works across hosts on the same Layer-2 broadcast
domain — DDS discovery uses UDP multicast `239.255.0.1` on port
`7400 + 250×domain_id`. If multicast is filtered (cloud, VLAN
without IGMP), set `RuntimeConfig.spdp_multicast_group` to a
unicast address you advertise via static peer-list (planned
config option).

## What now

- Define your own typed topic — see [04 IDL Reference](../04-idl/README.md).
- Pick a non-Rust binding — see [Integration](../README.md).
- Tune QoS for production — see [03 Configuration](../03-configuration/README.md).
