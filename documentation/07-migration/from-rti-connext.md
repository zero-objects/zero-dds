# Migration from RTI Connext DDS

What changes when an application moves from [RTI Connext][rti] to ZeroDDS,
and what stays. General points for every vendor are in the
[station README](README.md).

## Summary

| Area | Effort |
|---|---|
| IDL | unchanged; `@RTI_*` annotations are ignored |
| Modern C++ (`dds::…`, `<dds/dds.hpp>`) | same include from `crates/cpp`; `rti::…` extensions have no counterpart |
| C (`DDS_*`), legacy C++ (`DDSDomainParticipantFactory`) | rewritten onto `zerodds.h` or `<dds/dds.hpp>` |
| Java (`com.rti.dds.*`) | rewritten onto `org.omg.dds.*` (`crates/java-omgdds`) |
| `USER_QOS_PROFILES.xml` | read in Rust; RTI extensions are listed with `--skip-unknown`, RTI's own spellings with `--vendor-forms` |
| Discovery and transport in `<participant_qos>` | carried over by hand into `zerodds.yaml` |
| Mixed deployment | UDPv4 on the RTI side, see below |

## IDL

`keylist`, `//@key`, `@key`, `@nested`, `@topic` and `@default_nested` are
read. `@RTI_*` and `@allowed_data_representation` are ignored; a data
representation fixed by annotation is set in ZeroDDS through
`DataWriterQos.data_representation` or `wire.data_representation_offer`.

Under XCDR1, RTI counts `wstring` and `wchar` in characters, writes a
terminator and uses 4-octet units (`docs/wire-axis-flags.md`); ZeroDDS writes
the specification's form and has no setting for RTI's. Avoid those members, or
use XCDR2, where both sides must encode identically.

## C++ API

RTI Modern C++ and ZeroDDS both implement DDS-PSM-Cxx 1.0 under
`#include <dds/dds.hpp>`:

```cpp
dds::domain::DomainParticipant participant(0);
dds::topic::Topic<Robot::Pose> topic(participant, "Telemetry");
dds::pub::Publisher publisher(participant);
dds::pub::DataWriter<Robot::Pose> writer(publisher, topic);
writer.write(pose);
```

Differences: `rti::…` namespaces have no counterpart; there are no separate
headers `dds/pub/DataWriter.hpp` or `dds/sub/DataReader.hpp`, and no
`dds::core::QosProvider`. Generate the types with `zerodds-idlc generate
--lang c++` instead of `rtiddsgen`.

## QoS (`USER_QOS_PROFILES.xml`)

The file has the OMG DDS-XML form and is read by
`zerodds_xml::QosProfileRegistry::from_file` in Rust. `is_default_qos` is not
evaluated, and RTI extensions (`<transport_builtin>`, `<protocol>`,
participant-level `<resource_limits>`, `<representation>`) are not modelled:
the load ends with an error that names the element (for example
`participant_qos/resource_limits`). Remove the element from the file, or read it with
`zerodds-admin --skip-unknown qos check <file.xml>` /
`QosProfileRegistry::from_file_with(path, &ReadOptions::report_and_skip())`:
each element left out is listed with its path (`qos_profile[P]/datawriter_qos/publish_mode`).
Carry those policies over by hand.
The container `<participant_qos>` is RTI's spelling; DDS-XML 1.0 names it
`domain_participant_qos`, and the default reading refuses it with that
name. `zerodds-admin --vendor-forms …` /
`ReadOptions::accept_vendor_forms()` reads it and reports it with its path
(other vendor forms as well: `<topic_filter>` as an element, `qos_profile_ref`,
IDL type names such as `long`).
Enumeration values are the IDL literals of the DCPS IDL
(`RELIABLE_RELIABILITY_QOS`, `KEEP_LAST_HISTORY_QOS`); the short forms
(`RELIABLE`, `KEEP_LAST`) are refused. `NDDS_QOS_PROFILES` is not read.

## Configuration

| RTI | `zerodds.yaml` |
|---|---|
| `<discovery><initial_peers>`, `NDDS_DISCOVERY_PEERS` | `discovery.peers` |
| `<transport_builtin><mask>` | `transport.user_transport` / `transport.user_transports` |
| `<discovery><multicast_receive_addresses>` | `discovery.spdp_multicast_group` |
| `<participant_liveliness_lease_duration>` | `discovery.participant_lease_duration_ms` |
| Security plugins via `<property>` | `security.enclave_dir`, or the security API |

## Mixed deployment

- On one host RTI prefers shared memory; a participant of another stack then
  sees it, matches and receives no sample. Set `transport_builtin` to `UDPv4`
  on the RTI side.
- An RTI reader accepts XCDR1 by default, which is what a ZeroDDS writer
  writes. A ZeroDDS writer set to `[XCDR2]` needs `XCDR2_DATA_REPRESENTATION`
  on the RTI reader.
- GROUP-scope coherent sets from ZeroDDS to Connext 7.7.0 do not pass the OMG
  suite's `Test_CoherentSets_12` (`_21` passes).
- RTI needs `NDDSHOME` to start a participant.

## Tooling

| RTI | ZeroDDS |
|---|---|
| `rtiddsgen` | `zerodds-idlc` |
| `rtiddsspy` | `zerodds-spy` |
| `rtiddsperf` | `zerodds-perf` |
| Routing Service, Persistence Service | `zerodds-router`, `zerodds-durability-svc` (own configuration formats) |
| Security credential tooling | `zerodds-secure-permissions` |

## Reading further

- [`crates/idl-parse/tests/fixtures/rti/`](../../crates/idl-parse/tests/fixtures/rti/)
  — RTI IDL the parser is tested with.
- [`docs/wire-axis-flags.md`](../../docs/wire-axis-flags.md) — where RTI's
  wire form differs from the specification.

[rti]: https://www.rti.com/products/connext-dds-professional
