# Migration from Eclipse Cyclone DDS

[Cyclone DDS][cyclonedds] is the closest API neighbour to ZeroDDS
in spirit — both are Apache-licensed, OMG-spec-faithful, and
prioritise minimalism. This guide walks the typical port.

## TL;DR

| Step | Effort |
|---|---|
| IDL files port unchanged | none |
| Wire is byte-identical | none |
| Replace `cyclonedds.h` calls with `zerodds.h` (C) | mechanical |
| Replace `dds-c++` headers with `zerodds/zerodds.hpp` | mechanical, similar idioms |
| QoS XML — convert to OMG `zerodds-xml-1.0` | scriptable |
| `cyclonedds_init`-style configuration via `CYCLONEDDS_URI` env | manual, see below |

## IDL

Cyclone DDS uses standard OMG IDL 4.2 + the customary XTypes 1.3
annotations. Every IDL file ZeroDDS ships an idl-fixture for is
parsed unchanged:

```bash
cargo test -p zerodds-idl --test parse_vendors cyclone
```

Cyclone-specific generator pragmas (`#pragma keylist`, the
older `//@Key` comment style) are accepted by `zerodds-idlc` for
backward compatibility — they map onto `@key` annotations.

```idl
// Cyclone-original
#pragma keylist Trade ts symbol
struct Trade {
    long ts;
    string symbol;
    double price;
};

// Equivalent for ZeroDDS native (no migration needed if you keep the pragma)
@final
struct Trade {
    @key long ts;
    @key string symbol;
    double price;
};
```

## C API mapping

Cyclone-DDS C exposes `dds_*` functions with entity-handle return
codes. ZeroDDS' C-FFI exposes opaque pointers + status codes —
shape is similar enough that a 1-to-1 mapping is feasible.

| Cyclone DDS | ZeroDDS |
|---|---|
| `dds_create_participant(domain, qos, listener)` | `zerodds_runtime_create(domain)` |
| `dds_create_topic(participant, &desc, name, qos, listener)` | implicit — topic is identified by `(name, type)` strings on writer/reader create |
| `dds_create_publisher(participant, qos, listener)` | (no separate Publisher entity in `zerodds.h`; created internally per writer) |
| `dds_create_writer(publisher, topic, qos, listener)` | `zerodds_writer_create(rt, topic, type, reliable)` |
| `dds_create_reader(subscriber, topic, qos, listener)` | `zerodds_reader_create(rt, topic, type, reliable)` |
| `dds_write(writer, sample)` | `zerodds_writer_write(w, bytes, len)` |
| `dds_take(reader, &samples, &info, max, mask)` | `zerodds_reader_take(r, &out_buf, &out_len)` |
| `dds_delete(handle)` | `zerodds_*_destroy(handle)` |
| `dds_wait_for_acknowledgments(writer, timeout)` | (TODO — `zerodds_writer_wait_for_acks` planned) |

Non-trivial differences:

- ZeroDDS has no separate `Publisher`/`Subscriber` entity in the
  C-FFI — they are subsumed by writer/reader create. If your
  Cyclone code uses partition-on-the-publisher, set the partition
  on each writer instead.
- ZeroDDS' C-FFI is **byte-oriented** — the IDL serializer lives
  in your application or the C++ wrapper, not behind the FFI.
  Cyclone in contrast generates a `dds_topic_descriptor_t` that
  the C lib uses for serialization. With ZeroDDS, run
  `zerodds-idlc Robot.idl --cpp` and use the generated encoder.

## C++ API

Cyclone's C++ binding is `dds-cxx` (modern C++14 with
`org.omg.dds`-style classes). ZeroDDS' equivalent is
[`crates/cpp/`](../../crates/cpp/) — RAII-based, C++17 minimum.

```cpp
// Cyclone:
auto participant = dds::domain::DomainParticipant(domain_id);
auto topic       = dds::topic::Topic<Robot::Pose>(participant, "Telemetry");
auto pub         = dds::pub::Publisher(participant);
auto writer      = dds::pub::DataWriter<Robot::Pose>(pub, topic);
writer.write(pose);

// ZeroDDS (current C++ wrapper API):
auto rt = zerodds::Runtime::create(domain_id);
auto w  = rt->create_typed_writer<Robot::Pose>("Telemetry");
w->write(pose);
```

The OMG-modern-C++ PSM (`dds::pub::DataWriter<>`) shape is on the
ZeroDDS roadmap as a thin layer on top of the current API — when
shipped, the migration becomes a header-include change.

## QoS configuration

Cyclone reads QoS from `CYCLONEDDS_URI` pointing at an XML file:

```bash
export CYCLONEDDS_URI="file:///etc/cyclonedds.xml"
```

ZeroDDS uses `RuntimeConfig` in code. For now there's no
file-based config loader; planned via `zerodds-xml-wire`-driven
DDS-XML 1.0 file in a follow-up sprint.

Conversion script `scripts/cyclone_to_dds_xml.py` (planned) maps
the most common `CycloneDDS/Domain/Internal/...` knobs to
ZeroDDS `RuntimeConfig` fields.

## ROS-2

Cyclone is the ROS-2 default RMW (`rmw_cyclonedds_cpp`). Switch
to ZeroDDS by setting:

```bash
export RMW_IMPLEMENTATION=rmw_zerodds
```

See [05 Integration → ROS-2](../05-integration/ros2.md). No code
changes needed.

## Tooling

| Cyclone tool | ZeroDDS equivalent |
|---|---|
| `idlc` | `zerodds-idlc` |
| `ddsperf` | `zerodds-perf` (latency) + `roundtrip-1us` (sub-µs latency) |
| `rosbag2` for ROS-2 | `zerodds-replay` (zerodds-recording format) |
| `tcpdump` + custom dissector | `zerodds-traceability` |

## Test the migration

Stand up one Cyclone publisher and one ZeroDDS subscriber on the
same domain:

```bash
# Terminal 1 (Cyclone DDS):
ddsperf pub size 64 rate 1000

# Terminal 2 (ZeroDDS — same domain):
zerodds-admin --listen --topic PingPong
```

The ZeroDDS subscriber should see the Cyclone publisher's
samples without any cross-vendor configuration.

## Reading further

- [Cyclone DDS quickstart][cyclonedds] — for the source side.
- [`crates/idl/tests/fixtures/cyclonedds/`](../../crates/idl/tests/fixtures/cyclonedds/)
  — IDL-parser test fixtures pinned against Cyclone files.
- [`docs/interop/`](../../docs/interop/) — daily cross-vendor
  smoke tests including Cyclone.

[cyclonedds]: https://cyclonedds.io/
