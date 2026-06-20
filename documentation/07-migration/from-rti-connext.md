# Migration from RTI Connext DDS

[RTI Connext DDS Pro][rti] is the commercial reference DDS
implementation. The most common migration drivers to ZeroDDS:
licence cost, vendor lock-in, supply-chain audit, or memory-safety
mandate.

## TL;DR

| Step | Effort |
|---|---|
| IDL files port — most pragmas accepted, some rewrite needed | low |
| Wire is byte-identical | none |
| Replace `dds/dds.hpp` (Modern C++ PSM) — close but not identical | medium |
| QoS profile XMLs → OMG-DDS-XML 1.0 (subset) | scriptable |
| RTI Connext extensions (Connext-only QoS, RTI-Distributed-Logger, …) | code rewrite |
| `rtiddsgen` → `zerodds-idlc` | mechanical |

## IDL

RTI's IDL grammar is OMG-spec-superset with a handful of
extensions:

- `@RTI_*` annotations — vendor-specific, ignored by `zerodds-idlc`.
- `valuetype` (CORBA-IDL bridge) — supported by `zerodds-idlc`
  via the CORBA-coexistence path.
- Bitfield syntax `: N` — supported.

Test fixture: `crates/idl/tests/fixtures/rti/`. If your IDL uses
RTI-specific annotations and you need them honoured, file an
issue — most are silently dropped today.

## C++ API — Modern C++ PSM

RTI's "Modern C++ PSM" (`dds/dds.hpp`) is the OMG-spec-defined
header layout. ZeroDDS' C++ wrapper does **not yet** ship this
exact PSM — it's on the roadmap, but the current API is
`zerodds/zerodds.hpp` (RAII over the C-FFI).

```cpp
// RTI Connext Modern C++ PSM:
dds::domain::DomainParticipant participant(0);
dds::topic::Topic<HelloWorld> topic(participant, "HelloTopic");
dds::pub::Publisher pub(participant);
dds::pub::DataWriter<HelloWorld> writer(pub, topic);
HelloWorld sample("hello");
writer.write(sample);

// ZeroDDS today:
auto rt = zerodds::Runtime::create(0);
auto w  = rt->create_typed_writer<HelloWorld>("HelloTopic");
w->write(HelloWorld{"hello"});
```

The modern C++ PSM layer is on track; until shipped, the
migration costs ~30 lines of API translation per typical
application.

## C API

RTI's C API is `DDS_*`-prefixed (similar to legacy OpenDDS).
Mapping to ZeroDDS' `zerodds_*` is mechanical — see the table
in [from-cyclonedds.md](from-cyclonedds.md) which is largely
identical.

## QoS XML

RTI ships extensive QoS XML support (`USER_QOS_PROFILES.xml`).
The schema is OMG-DDS-1.0 (the older version) plus RTI
extensions.

ZeroDDS will accept OMG-DDS-XML 1.0 schema files
(`crates/zerodds-xml-wire/`) — converter from RTI XML is planned via
`scripts/rti_xml_to_dds_xml.py` (TODO). Manual translation tips:

| RTI element | OMG/ZeroDDS equivalent |
|---|---|
| `<qos_profile name="X" base_name="Y">` | OMG `<qos_profile name="X">` (no base_name; flatten manually) |
| `<datawriter_qos>` | `<datawriter_qos>` |
| `<reliability><kind>RELIABLE…</kind></reliability>` | same |
| `<protocol><rtps_reliable_writer><heartbeat_period>` | `ReliableWriterConfig.heartbeat_period` (in code) |
| `<resource_limits><max_samples>` | `ResourceLimitsQosPolicy.max_samples` |
| `<zerodds_security>...` | governance.xml + permissions.xml in OMG-format (different schema!) |

## Vendor-specific QoS extensions

RTI Connext Pro has many policies not in OMG-DDS-1.4:

| RTI policy | ZeroDDS strategy |
|---|---|
| `transport_priority` (DSCP-marking) | manual via `setsockopt` on the user_unicast socket; planned `RuntimeConfig.transport_priority` |
| `multi_channel` | not implemented; achieve same via per-topic-domain split |
| `availability` | not implemented; use `Durability.TransientLocal` for the same effect |
| `database` (in-memory persistence) | not implemented |
| `discovery_config.builtin_discovery_plugins` | only SPDP+SEDP today; static-peer-list planned |
| `topic_query` | not in OMG spec; not implemented |

If your code depends on these extensions, the migration is
non-trivial — file an issue with your specific extension list.

## Security

RTI's Security plugins (Pro edition, separate licence) implement
DDS-Security 1.1. ZeroDDS implements full DDS-Security 1.2 with
no extra licence.

Cert reuse:

- Identity CA + Permissions CA → reuse unchanged.
- Identity certs → reuse unchanged.
- Governance + Permissions XML → reuse the OMG-spec format
  unchanged. RTI uses the same OMG schema since 5.3.x.

The only delta is plugin loading:

```xml
<!-- RTI: -->
<participant_qos>
    <property>
        <value>
            <element>
                <name>com.rti.serv.load_plugin</name>
                <value>com.rti.serv.secure</value>
            </element>
        </value>
    </property>
</participant_qos>

<!-- ZeroDDS: enable feature in Cargo + inject SharedSecurityGate in code. -->
```

## Routing & Cloud

RTI Connext Cloud Discovery Service / Routing Service have no
direct ZeroDDS equivalent in 0.0.0. Workarounds:

- Cloud-Discovery-Service equivalent: planned static-peer-list
  config + future `zerodds-discovery-server` tool.
- Routing-Service equivalent: write a small bridge using two
  DcpsRuntime instances on different domains, plus
  topic-name remapping in user code.

## Tooling

| RTI tool | ZeroDDS equivalent |
|---|---|
| `rtiddsgen` | `zerodds-idlc` |
| `rtiddsspy` | `zerodds-admin --listen` |
| `rtiddsperf` | `zerodds-perf` + `roundtrip-1us` |
| RTI Admin Console | `tools/dashboard` (Tauri, in progress) |
| RTI System Designer | not yet; use RTI System Designer to design, export OMG-DDS-XML, then load into ZeroDDS |

## Licensing

| Aspect | RTI | ZeroDDS |
|---|---|---|
| Per-seat dev | yes (Connext Pro) | none — Apache 2.0 |
| Per-deployment runtime | yes | none |
| Source available | partial | full |
| Audit-ready supply-chain | depends | full Cargo dep tree, all Apache 2.0 |

## Reading further

- [RTI Connext DDS docs][rti]
- [`crates/idl/tests/fixtures/rti/`](../../crates/idl/tests/fixtures/rti/)
  — IDL-parser regression corpus.

[rti]: https://www.rti.com/products/connext-dds-professional
