# Migration from OpenDDS

What changes when an application moves from [OpenDDS][opendds] to ZeroDDS,
and what stays. General points for every vendor are in the
[station README](README.md).

## Summary

| Area | Effort |
|---|---|
| IDL | unchanged; `#pragma DCPS_DATA_KEY` keys the named member |
| C++ (TAO mapping, `_var` types, `TheParticipantFactory`) | rewritten onto `<dds/dds.hpp>` or `zerodds.h` |
| `opendds_idl` + `tao_idl` | `zerodds-idlc generate --lang c++` |
| QoS | in code, as in OpenDDS |
| `-DCPSConfigFile` `.ini` | carried over by hand into `zerodds.yaml` |
| Mixed deployment | RTPS discovery and `rtps_udp` on the OpenDDS side |

## IDL

OpenDDS IDL is read unchanged, including its pragmas:

```idl
#pragma DCPS_DATA_TYPE "Robot::Pose"
#pragma DCPS_DATA_KEY  "Robot::Pose id"

module Robot {
    struct Pose {
        string id;
        double x;
        double y;
        double z;
    };
};
```

`DCPS_DATA_KEY` makes the named member a key, in the quoted form above and
unquoted (`Robot::Pose id`). A key path into a member (`pos.x`, `arr[0]`) is
refused: keying the outer member would key all of it. Write `@key` on the
member of the inner type instead. A field name the type does not declare is
reported.

OpenDDS ignores `@value` on enumerators and `@bit_bound`, and frames unions
and `@mutable` member headers differently under XCDR1
(`docs/wire-axis-flags.md`); there is no runtime setting for those on the
ZeroDDS side.

## C++ API

OpenDDS uses TAO's IDL-to-C++ mapping (`DDS::DomainParticipantFactory_var dpf
= TheParticipantFactoryWithArgs(argc, argv)`, `_var` types, generated
`TypeSupport` classes). None of it exists in ZeroDDS. The target is
DDS-PSM-Cxx:

```cpp
#include <dds/dds.hpp>

dds::domain::DomainParticipant participant(0);
dds::topic::Topic<Robot::Pose> topic(participant, "Movie");
dds::pub::Publisher publisher(participant);
dds::pub::DataWriter<Robot::Pose> writer(publisher, topic);
writer.write(pose);
```

What changes is entity creation, memory management (`_var` becomes RAII) and
type support. Java applications move to `org.omg.dds.*` in
`crates/java-omgdds`; there is no JMS layer.

## Configuration (`-DCPSConfigFile`)

| OpenDDS | ZeroDDS |
|---|---|
| `[common] DCPSDefaultDiscovery=rtps_disc` | required on the OpenDDS side: its default is InfoRepo |
| `[transport/…] transport_type=rtps_udp` | required on the OpenDDS side |
| `[rtps_discovery/…] use_xtypes=no` | needed when ZeroDDS writers announce no TypeObject (the plain `*_create` C functions); not with the `*_typed` ones |
| `SpdpSendAddrs`, static peers | `discovery.peers` |
| `ResendPeriod`, `LeaseDuration` | `discovery.spdp_period_ms`, `discovery.participant_lease_duration_ms` |
| InfoRepo, RtpsRelay | no counterpart |

## Mixed deployment

- OpenDDS checks the encapsulation header strictly; ZeroDDS sets it from the
  declared extensibility.
- An OpenDDS reader that accepts only XCDR2 needs
  `wire.data_representation_offer: [XCDR2]` on the ZeroDDS writer.
- The authentication handshake NUL-terminates the algorithm names for an
  OpenDDS peer, as OpenDDS requires; this follows from the peer's vendor id,
  no setting.
- Secure SPDP with OpenDDS needs the default `wire.rtps_dialect`
  (`maximally-interoperable`): it answers the beacon's source address as well,
  which OpenDDS 3.34 is reached on.

## Tooling

| OpenDDS | ZeroDDS |
|---|---|
| `opendds_idl`, `tao_idl` | `zerodds-idlc` |
| InfoRepo | none; RTPS discovery only |
| `inspect`-style monitoring | `zerodds-spy`, `zerodds-admin discovery snapshot <domain>` |

## Reading further

- [`docs/wire-axis-flags.md`](../../docs/wire-axis-flags.md) — where OpenDDS's
  wire form differs from the specification.
- [`tests/perf/dds-roundtrip-bench/opendds_rtps.ini`](../../tests/perf/dds-roundtrip-bench/opendds_rtps.ini)
  — an OpenDDS configuration used against ZeroDDS.

[opendds]: https://opendds.org/
