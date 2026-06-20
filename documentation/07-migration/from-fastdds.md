# Migration from eProsima Fast DDS

[Fast DDS][fastdds] (formerly Fast-RTPS) is the second ROS-2-blessed
RMW backend and a popular choice for industrial automation.
ZeroDDS is wire-compatible with Fast DDS and ports cleanly.

## TL;DR

| Step | Effort |
|---|---|
| IDL files port unchanged | none |
| Wire is byte-identical | none |
| Replace `<fastdds/dds/...>` headers with `<zerodds/zerodds.hpp>` | mechanical |
| `fastdds.xml` profiles → `RuntimeConfig` + per-endpoint QoS | scriptable |
| `Fast-DDS-Gen` → `zerodds-idlc` for codegen | mechanical |

## IDL

Fast DDS uses standard OMG IDL 4.2. `zerodds-idlc` parses Fast-DDS IDL
files unchanged — see
`crates/idl/tests/fixtures/fastdds/` for the regression corpus.

Differences:

- Fast-DDS-Gen generates type-support classes with names like
  `RobotPubSubType` (for type `Robot`). ZeroDDS' codegen produces
  one type class per IDL type, no separate "PubSubType" wrapper.
- Fast-DDS extends IDL with `@extensibility(MUTABLE)`-style
  hints; the canonical OMG spelling is `@mutable`. Both work.

## C++ API

Fast DDS exposes a "modern C++11" API rooted in
`eprosima::fastdds::dds`:

```cpp
// Fast DDS:
auto factory = DomainParticipantFactory::get_instance();
DomainParticipant* p = factory->create_participant(0, PARTICIPANT_QOS_DEFAULT);
TypeSupport ts(new HelloWorldPubSubType());
ts.register_type(p, "HelloWorld");
Topic* topic = p->create_topic("HelloTopic", "HelloWorld", TOPIC_QOS_DEFAULT);
Publisher* pub = p->create_publisher(PUBLISHER_QOS_DEFAULT);
DataWriter* w = pub->create_datawriter(topic, DATAWRITER_QOS_DEFAULT);
HelloWorld msg;  msg.message("hello");
w->write(&msg);

// ZeroDDS (current C++ wrapper):
auto rt = zerodds::Runtime::create(0);
auto w  = rt->create_typed_writer<HelloWorld>("HelloTopic");
HelloWorld msg;
msg.message = "hello";
w->write(msg);
```

Fast DDS' `TypeSupport` registration step is implicit in ZeroDDS —
the typed writer template parameter carries the type-name.

## QoS XML profiles

Fast DDS reads XML profiles via `DEFAULT_FASTRTPS_PROFILES.xml`
or `FASTRTPS_DEFAULT_PROFILES_FILE`. The file describes
participant + writer + reader QoS per profile name.

ZeroDDS does not yet consume XML profiles in the runtime —
configure via `RuntimeConfig` and per-endpoint `UserWriterConfig`
in code. A converter `scripts/fastdds_xml_to_rust.py` (planned)
emits Rust constructor calls from a Fast-DDS profile XML.

The most-used profile elements map to:

| Fast-DDS XML | ZeroDDS field |
|---|---|
| `<participant>/<rtps>/<defaultUnicastLocatorList>` | `RuntimeConfig.interface_bindings` |
| `<data_writer>/<qos>/<reliability>` | `UserWriterConfig.reliable` |
| `<data_writer>/<qos>/<durability>` | `UserWriterConfig.durability` |
| `<data_writer>/<qos>/<deadline>` | `UserWriterConfig.deadline` |
| `<data_writer>/<topic>/<historyQos>` | `ReliableWriterConfig.history_kind` (via factory) |
| `<security>/<authentication>` | `RuntimeConfig.security` (with the `security` cargo feature) |

## Discovery

Fast DDS uses SPDP on `239.255.0.1:7400 + 250×D` — same as
ZeroDDS. They auto-discover each other on the same L2 broadcast
domain.

Fast DDS also ships a "discovery server" mode for
unicast-only environments. ZeroDDS' equivalent (planned) is the
static peer-list option; until then, use UDP routing or a TCP
discovery bridge.

## Security

Fast DDS' security plugins are vendor-built-in (DDS-Security 1.1
+ extensions). ZeroDDS implements full DDS-Security 1.2.

Migration of certs:

- Identity CA + Permissions CA — same X.509 certs work; reuse them.
- Governance + Permissions XML — same OMG schema; reuse them
  unchanged.
- S/MIME-signed permissions XML — same format
  (`openssl smime -sign`).

Difference: Fast DDS exposes plugin-loading via cmake-config
(`FASTDDS_SECURITY=ON`). ZeroDDS gates the security stack on the
`security` cargo feature plus a `RuntimeConfig.security: Some(gate)`
injection.

## ROS-2

Fast DDS is the ROS-2 default since Foxy. Switch to ZeroDDS:

```bash
export RMW_IMPLEMENTATION=rmw_zerodds
```

The ROS-2 plugin is `rmw-zerodds-shim`.

## Tooling

| Fast-DDS tool | ZeroDDS equivalent |
|---|---|
| `Fast-DDS-Gen` (IDL compiler) | `zerodds-idlc` |
| `fastdds discovery` (CLI) | `zerodds-admin --participants` |
| `fastdds shm clean` | shm segments are auto-cleaned on process exit |
| `Wireshark RTPS plugin` | `zerodds-traceability` (cli) + Wireshark works unchanged |

## Test the migration

```bash
# Terminal 1 (Fast DDS):
fastdds discovery server -i 0  # if using discovery server, optional

# Terminal 2 (Fast DDS publisher demo):
HelloWorldExamplePublisher

# Terminal 3 (ZeroDDS subscriber on the same domain):
zerodds-admin --listen --topic HelloWorldTopic
```

Cross-vendor wire-compat smoke runs nightly under
`docs/interop/` against the latest Fast DDS release.

## Reading further

- [Fast DDS docs][fastdds]
- [`crates/idl/tests/fixtures/fastdds/`](../../crates/idl/tests/fixtures/fastdds/)
- ROS-2 RMW comparison docs at
  [design.ros2.org](https://design.ros2.org/articles/ros_middleware_interface.html)

[fastdds]: https://fast-dds.docs.eprosima.com/
