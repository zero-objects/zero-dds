# Migration from eProsima Fast DDS

What changes when an application moves from [Fast DDS][fastdds] to ZeroDDS,
and what stays. General points for every vendor are in the
[station README](README.md).

## Summary

| Area | Effort |
|---|---|
| IDL | unchanged |
| C++ (`eprosima::fastdds::dds::*`) | rewritten onto `<dds/dds.hpp>` (DDS-PSM-Cxx) or `<zerodds/dds.hpp>` |
| `fastddsgen` | `zerodds-idlc generate --lang c++` |
| XML profiles (`<profiles>`) | not read; QoS in code or rewritten as DDS-XML |
| `FASTDDS_DEFAULT_PROFILES_FILE` configuration | carried over by hand into `zerodds.yaml` |
| Mixed deployment | standard discovery, no extra configuration |

## IDL

Fast DDS IDL is read unchanged. Where Fast DDS's wire form differs from the
specification (`docs/wire-axis-flags.md`), the ZeroDDS side writes the
specification's form, and there is no runtime setting to change that:
Fast DDS drops `@bit_bound` on enumerations, orders map entries by key and
counts `wstring` in characters. IDL that both sides must encode identically
avoids those constructs: 32-bit enumerations, no `map`, `string` rather than
`wstring`.

## C++ API

The Fast DDS API has no counterpart. The DCPS entities keep their names; what
changes is the namespace, creation (constructors and RAII instead of `create_*`
returning raw pointers) and type support (`TopicDataType`/`TypeSupport` become
the generated type traits):

```cpp
// Fast DDS:
DomainParticipant* p = DomainParticipantFactory::get_instance()
    ->create_participant(0, PARTICIPANT_QOS_DEFAULT);
Publisher* pub = p->create_publisher(PUBLISHER_QOS_DEFAULT);

// ZeroDDS (DDS-PSM-Cxx, #include <dds/dds.hpp>):
dds::domain::DomainParticipant participant(0);
dds::topic::Topic<HelloWorld> topic(participant, "HelloWorldTopic");
dds::pub::Publisher publisher(participant);
dds::pub::DataWriter<HelloWorld> writer(publisher, topic);
writer.write(sample);
```

Generate `HelloWorld` with `zerodds-idlc generate --lang c++ --out gen/
HelloWorld.idl`.

## QoS

Fast DDS XML (`<profiles>`) is not read: `<profiles>` as root is refused, and
inside `<dds>` it yields no library. Carry the profiles into code or rewrite
them as OMG DDS-XML (`<dds><qos_library><qos_profile>`), which
`zerodds_xml::QosProfileRegistry` reads in Rust and `zerodds-admin qos check`
verifies.

## Configuration

ZeroDDS does not read Fast DDS profile files. The common settings in
[`zerodds.yaml`](../../man/man5/zerodds.yaml.5):

| Fast DDS | `zerodds.yaml` |
|---|---|
| `<initialPeersList>` | `discovery.peers` |
| `<metatrafficUnicastLocatorList>`, `<interfaceWhiteList>` | `transport.interface`, `transport.interface_bindings` |
| `<useBuiltinTransports>false` with a UDPv4 transport | `transport.user_transport: UDPv4` (the default) |
| `<leaseDuration>` | `discovery.participant_lease_duration_ms` |
| Discovery Server client (`<discoveryProtocol>CLIENT`) | no client for the Fast DDS Discovery Server protocol; ZeroDDS has its own SPDP relay, `discovery.server` |

## Mixed deployment

Fast DDS and ZeroDDS discover each other with the defaults. ROS 2 services
with `rmw_fastrtps_cpp` need no setting: ZeroDDS writes the related sample
identity both as `PID_RELATED_SAMPLE_IDENTITY` and as eProsima's `0x800f`,
the only form Fast DDS 2.6 reads. DDS-Security with Fast DDS needs RSA identity
certificates.

## ROS 2

Build the ament package `rmw_zerodds_cpp`
([`crates/rmw-zerodds-shim`](../../crates/rmw-zerodds-shim/README.md)) and
select it:

```bash
export RMW_IMPLEMENTATION=rmw_zerodds_cpp
```

Topics and services run across `rmw_zerodds_cpp` and `rmw_fastrtps_cpp`.

## Tooling

| Fast DDS | ZeroDDS |
|---|---|
| `fastddsgen` | `zerodds-idlc` |
| `fastdds discovery` (Discovery Server) | `discovery.server` (own SPDP relay, not the Fast DDS protocol) |
| Fast DDS Monitor | `zerodds-monitor`, `zerodds-spy` |

## Reading further

- [`crates/idl-parse/tests/fixtures/fastdds/`](../../crates/idl-parse/tests/fixtures/fastdds/)
  — Fast DDS IDL the parser is tested with.
- [`docs/wire-axis-flags.md`](../../docs/wire-axis-flags.md) — where Fast DDS's
  wire form differs from the specification.

[fastdds]: https://www.eprosima.com/index.php/products-all/eprosima-fast-dds
