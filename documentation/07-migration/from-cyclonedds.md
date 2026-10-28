# Migration from Eclipse Cyclone DDS

What changes when an application moves from [Cyclone DDS][cyclonedds] to
ZeroDDS, and what stays. General points for every vendor are in the
[station README](README.md).

## Summary

| Area | Effort |
|---|---|
| IDL | unchanged; pass `--default-extensibility final` or annotate the types |
| C (`dds_*`) | rewritten onto `zerodds.h`, one call for one call |
| C++ (Cyclone-CXX, `dds::…`) | `#include <dds/dds.hpp>` from `crates/cpp` |
| QoS | DDS-XML profiles are read in Rust; otherwise in code |
| `CYCLONEDDS_URI` configuration | carried over by hand into `zerodds.yaml` |
| Mixed deployment | standard discovery, no extra configuration |

## IDL

Cyclone IDL is read unchanged. `#pragma keylist` makes the named members keys:

```idl
#pragma keylist Trade ts symbol
struct Trade {
    long ts;
    string symbol;
    double price;
};
```

Cyclone's `idlc` takes an aggregate without an extensibility annotation as
`@final`; `zerodds-idlc` takes it as `@appendable`, as the specification does.
Generate with the Cyclone default, or write `@final`:

```bash
zerodds-idlc generate --lang c++ --default-extensibility final --out gen/ Trade.idl
```

## C API

There is no `dds_*` API. The C API in `zerodds.h` has the DCPS structure under
its own names:

| Cyclone DDS | ZeroDDS |
|---|---|
| `dds_create_participant(domain, qos, listener)` | `zerodds_dpf_create_participant(zerodds_dpf_get_instance(), domain, qos)` |
| `dds_create_topic(pp, &desc, name, qos, listener)` | `zerodds_dp_create_topic(dp, name, type_name, qos)`, or `zerodds_topic_create_typed` with a TypeObject |
| `dds_create_publisher` / `dds_create_subscriber` | `zerodds_dp_create_publisher` / `zerodds_dp_create_subscriber` |
| `dds_create_writer` / `dds_create_reader` | `zerodds_pub_create_datawriter[_typed]` / `zerodds_sub_create_datareader[_typed]` |
| `dds_write(wr, &sample)` | `zerodds_dw_write(dw, bytes, len, handle)` |
| `dds_take` / `dds_read` | `zerodds_dr_take` / `zerodds_dr_read`, then `zerodds_dr_return_loan` |
| `dds_wait_for_acks` | `zerodds_dw_wait_for_acknowledgments` |
| `dds_delete(e)` | `zerodds_dpf_delete_participant`, `zerodds_pub_delete_datawriter`, … |

Cyclone serialises through the `dds_topic_descriptor_t` its `idlc` generates.
ZeroDDS takes CDR bytes: the encoder is the code `zerodds-idlc generate
--lang c` writes. The plain `*_create` functions announce no TypeObject, so a
peer matches by topic and type name; the `*_typed` variants pass one.

## C++ API

Cyclone-CXX and ZeroDDS both implement DDS-PSM-Cxx 1.0. Code using
`dds::domain::DomainParticipant`, `dds::topic::Topic<T>`,
`dds::pub::Publisher`, `dds::pub::DataWriter<T>` and their `sub` counterparts
compiles against `#include <dds/dds.hpp>` from `crates/cpp`. Two differences:
ZeroDDS has no separate headers `dds/pub/DataWriter.hpp` or
`dds/sub/DataReader.hpp` — include `<dds/dds.hpp>` — and no
`dds::core::QosProvider`.

## QoS

A DDS-XML file (`<dds><qos_library><qos_profile>`) is read by
`zerodds_xml::QosProfileRegistry::from_file` in Rust and checked by
`zerodds-admin qos check <file.xml>`. The other bindings take QoS in code.

## Configuration (`CYCLONEDDS_URI`)

ZeroDDS does not read the Cyclone XML configuration. The common settings in
[`zerodds.yaml`](../../man/man5/zerodds.yaml.5):

| Cyclone | `zerodds.yaml` |
|---|---|
| `General/NetworkInterfaceAddress`, `General/Interfaces/NetworkInterface` | `transport.interface` (one IPv4 address), `transport.interface_bindings` |
| `General/AllowMulticast=false` | `discovery.no_multicast: true` |
| `General/MaxMessageSize` | `transport.max_message_size` |
| `Discovery/Peers/Peer@address` | `discovery.peers` |
| `Discovery/MaxAutoParticipantIndex` | `discovery.max_peer_participants` |
| `Discovery/SPDPMulticastAddress` | `discovery.spdp_multicast_group` |
| `Discovery/SPDPInterval` | `discovery.spdp_period_ms` |
| `Discovery/Tag` | `discovery.domain_tag` — the same value on both sides |
| `Discovery/ParticipantIndex` | none; ZeroDDS takes the first free index |
| `Discovery/Ports/*` | none; the standard port formula is fixed |
| `Tracing/*` | `diagnostics.*` |

## Mixed deployment

Cyclone and ZeroDDS discover each other with the defaults. Two settings matter:

- A Cyclone reader that accepts only XCDR2 (`-x 2` in the OMG shape
  application) needs `wire.data_representation_offer: [XCDR2]` on the ZeroDDS
  writer.
- A domain tag on the Cyclone side (`Discovery/Tag`) needs the same
  `discovery.domain_tag`; with different tags, or a tag on one side only, the
  two do not see each other.

To see Cyclone's `ddsperf` samples arrive:

```bash
# Terminal 1 (Cyclone):
ddsperf -i 7 pub 20Hz

# Terminal 2 (ZeroDDS):
zerodds-spy -d 7 -t DDSPerfRDataKS
```

## ROS 2

Cyclone is the default RMW (`rmw_cyclonedds_cpp`). Build the ament package
`rmw_zerodds_cpp` ([`crates/rmw-zerodds-shim`](../../crates/rmw-zerodds-shim/README.md))
and select it:

```bash
export RMW_IMPLEMENTATION=rmw_zerodds_cpp
```

Topics and services run across `rmw_zerodds_cpp` and `rmw_cyclonedds_cpp`.

## Tooling

| Cyclone | ZeroDDS |
|---|---|
| `idlc` | `zerodds-idlc` |
| `ddsperf` | `zerodds-perf`, `zerodds-bench` |
| `cyclonedds ps` / `sub` | `zerodds-spy`, `zerodds-admin discovery snapshot <domain>` |

## Reading further

- [`crates/idl-parse/tests/fixtures/cyclonedds/`](../../crates/idl-parse/tests/fixtures/cyclonedds/)
  — Cyclone IDL the parser is tested with.
- [`docs/wire-axis-flags.md`](../../docs/wire-axis-flags.md) — where Cyclone's
  wire form differs from the specification.

[cyclonedds]: https://cyclonedds.io/
