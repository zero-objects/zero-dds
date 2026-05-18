# Migration from OpenDDS

[OpenDDS][opendds] is the long-running C++ DDS implementation
maintained by Object Computing Inc. It traces back to TAO/CORBA
and is widely used in defence and aerospace. ZeroDDS is wire-
compatible.

## TL;DR

| Step | Effort |
|---|---|
| IDL files port — TAO IDL → OMG IDL | low (mostly mechanical) |
| Wire is byte-identical (RTPS-2.5 + CDR) | none |
| Replace `<dds/DCPS/...>` headers — different namespaces and idioms | medium |
| OpenDDS `.ini` config → `RuntimeConfig` | manual (no XML/INI loader yet) |
| `opendds_idl` → `zerodds-idlc` | mechanical |

## IDL

OpenDDS uses standard OMG IDL with TAO-flavoured pragma syntax:

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

Both pragma styles parse with `zerodds-idlc`:

- `#pragma DCPS_DATA_TYPE "<name>"` — silently dropped (we
  derive the topic-type-name from the IDL type itself).
- `#pragma DCPS_DATA_KEY "<scope> <field>"` — mapped to `@key`
  on the named member.

The native ZeroDDS spelling is the XTypes-1.3-canonical form:

```idl
@final
struct Pose {
    @key string id;
    double x;
    double y;
    double z;
};
```

## C++ API

OpenDDS exposes both a "TAO-flavoured" PSM
(`OpenDDS::DCPS::*`) and the OMG-spec PSM
(`DDS::*`). Both are heavyweight CORBA-style interfaces.

```cpp
// OpenDDS TAO PSM:
DDS::DomainParticipantFactory_var factory = TheParticipantFactory;
DDS::DomainParticipant_var participant =
    factory->create_participant(0, PARTICIPANT_QOS_DEFAULT, 0,
                                OpenDDS::DCPS::DEFAULT_STATUS_MASK);
PoseTypeSupportImpl::_var_type ts = new PoseTypeSupportImpl();
ts->register_type(participant, "Pose");
DDS::Topic_var topic =
    participant->create_topic("Telemetry", "Pose", TOPIC_QOS_DEFAULT, 0,
                              OpenDDS::DCPS::DEFAULT_STATUS_MASK);
// … similar verbosity for publisher + writer …

// ZeroDDS:
auto rt = zerodds::Runtime::create(0);
auto w  = rt->create_typed_writer<Robot::Pose>("Telemetry");
w->write(pose);
```

The OpenDDS `_var` smart-pointer + `TheParticipantFactory`
singleton pattern doesn't carry over — ZeroDDS uses RAII +
explicit ownership.

## INI configuration

OpenDDS reads runtime config from `.ini` files via
`-DCPSConfigFile`. Typical content:

```ini
[common]
DCPSGlobalTransportConfig=$file
DCPSDebugLevel=1

[transport/the_rtps_transport]
transport_type=rtps_udp

[config/the_rtps_config]
transports=the_rtps_transport
```

ZeroDDS doesn't parse INI files — `RuntimeConfig` in code is the
source of truth. A converter is planned; until then, the most
common knobs translate as:

| OpenDDS INI | ZeroDDS field |
|---|---|
| `[common]/DCPSDebugLevel` | use `RUST_LOG=zerodds_dcps=debug` env |
| `[transport/x]/transport_type=rtps_udp` | this is the only ZeroDDS transport for discovery — implicit |
| `[transport/x]/use_multicast=1` | always on for SPDP |
| `[transport/x]/multicast_group_address` | `RuntimeConfig.spdp_multicast_group` |
| `[domain/0]/RtpsRelayAddress` | not implemented; use unicast static peer-list |
| `[domain/0]/Security` block | enable `security` cargo feature + inject `SharedSecurityGate` |

## OpenDDS-specific features

| Feature | ZeroDDS strategy |
|---|---|
| RtpsRelay (cloud relay) | not implemented; planned static-peer-list + future relay tool |
| Federation Service | not in OMG-DDS-1.4 spec scope |
| Persistence (`Persistent` durability) | available via `Durability::Persistent`, backend pluggable |
| Java + JMS bridge | use the Pure-Java DDS-Java-PSM (`zerodds-java-omgdds`) directly; no JMS layer in ZeroDDS |

## Security

OpenDDS implements DDS-Security 1.1 (built-in plugin). ZeroDDS
implements DDS-Security 1.2 with no licence delta.

Migration of certs:

- Identity CA + Permissions CA → reuse unchanged.
- Identity certs → reuse unchanged.
- Governance + Permissions XML → OMG schema, reuse unchanged.
- `permissions_ca` config in `.ini` → governance.xml + permissions
  XML loaded via `parse_governance_xml` /
  `parse_permissions_xml` in code.

## Tooling

| OpenDDS tool | ZeroDDS equivalent |
|---|---|
| `opendds_idl` | `zerodds-idlc` |
| `monitor` | `zerodds-admin --participants` (live view) |
| `inspect` | `zerodds-traceability` (offline pcap) |
| `repoctl` (federation) | not applicable |

## Test the migration

```bash
# Terminal 1 (OpenDDS):
DCPSConfigFile=rtps.ini ./MessengerPublisher

# Terminal 2 (ZeroDDS subscriber):
zerodds-admin --listen --topic Movie
```

The ZeroDDS subscriber should see the OpenDDS publisher's
samples — verifies wire compat.

## Reading further

- [OpenDDS Developer's Guide][opendds-guide]
- [`crates/idl/tests/fixtures/opendds/`](../../crates/idl/tests/fixtures/opendds/)

[opendds]: https://opendds.org/
[opendds-guide]: https://opendds.org/documentation
