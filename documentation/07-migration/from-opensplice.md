# Migration from Eclipse OpenSplice

What changes when an application moves from [OpenSplice][opensplice]
Community Edition 6.9 to ZeroDDS, and what stays. General points for every
vendor are in the [station README](README.md).

## Summary

| Area | Effort |
|---|---|
| IDL (IDL 3.5, `#pragma keylist`) | read unchanged by `zerodds-idlc` |
| ISO C++ (ISOCPP2, `dds::…`) | `#include <dds/dds.hpp>` from `crates/cpp` |
| Standalone C++ (SACPP/CCPP, `DDS::…`, `_var` types) | rewritten onto `<dds/dds.hpp>` or `zerodds.h` |
| C (SAC, `DDS_*`) | rewritten onto `zerodds.h` |
| Java 5 PSM (`org.omg.dds.*`) | `crates/java-omgdds` |
| `OSPL_URI` configuration (`ospl.xml`) | carried over by hand into `zerodds.yaml` |
| Shared-memory deployment (`ospl start`, federated mode) | no counterpart; every ZeroDDS participant is a process of its own |
| Mixed deployment | standard discovery; see below for what the two sides can say to each other |

## IDL

OpenSplice's `idlpp` reads OMG IDL 3.5: keys are declared with
`#pragma keylist`, annotations do not exist. `zerodds-idlc` reads such files
unchanged:

```idl
module Robot {
    struct Pose {
        string id;
        double x;
        double y;
    };
#pragma keylist Pose id
};
```

The directive is read the way `idlpp` reads it: in the module it stands in,
with members separated by spaces or commas (`#pragma keylist Pose id, x`),
and with a path into a nested struct (`#pragma keylist Outer inner.b`), which
makes `inner` a key of `Outer` and `b` the key of the struct `inner` has —
the form DDS-XTypes 1.3 §7.6.8 builds a nested key holder from. A nested
struct has one set of key members: a path that would give it a second, or
shrink the key of a member that keys by the whole struct, is refused.
Whether the KeyHash of a type with several keys of different widths matches
OpenSplice's has not been measured.

`zerodds-idlc` takes a struct without an extensibility annotation as
`@appendable`, as the specification does. Under XCDR1, which is the only
representation OpenSplice writes, `@appendable` and `@final` produce the same
bytes, so the default needs no flag for an OpenSplice peer.

The CDR dialect of OpenSplice is `opensplice`
(`zerodds-idlc generate --dialect opensplice`, `docs/wire-axis-flags.md`,
column `OSPL¹`). It gives the specification's answer on every axis:
recorded against the corpus in `tests/interop-corpus/`, OpenSplice's octets
are reproduced for 191 of 191 cells, 161 exactly and 30 up to the content of
alignment padding, which OpenSplice leaves unwritten.

What OpenSplice 6.9's IDL cannot express, and a type shared with an
OpenSplice peer therefore cannot use: `@mutable`, `@optional`, `@external`,
`@must_understand`, `@value`, an enum `@bit_bound` below 32, `wchar`,
`wstring`, `fixed`, `map`, `bitmask`, `bitset`, struct inheritance, empty
structs, `octet`/`int8`/`uint8` union discriminators, struct-typed key
members, and a union as the topic type. The refusals are recorded per case
under `tests/interop-results/sources/toolchain-logs/opensplice/`.

## C++ API

OpenSplice ISOCPP2 and ZeroDDS both implement DDS-PSM-Cxx 1.0. Code using
`dds::domain::DomainParticipant`, `dds::topic::Topic<T>`,
`dds::pub::DataWriter<T>` and `dds::sub::DataReader<T>` compiles against
`#include <dds/dds.hpp>` from `crates/cpp`; there are no separate headers per
entity and no `dds::core::QosProvider`.

The standalone C++ API (SACPP, the classic DCPS mapping with
`DDS::DomainParticipantFactory::get_instance()` and `_var` types) has no
counterpart and is rewritten onto one of the two APIs above. Generate the
types with `zerodds-idlc generate --lang c++` instead of `idlpp`.

## C API

The SAC API (`DDS_DomainParticipantFactory_get_instance`, `DDS_*`) maps onto
`zerodds.h` with the same DCPS structure; the table in
[from-cyclonedds.md](from-cyclonedds.md) lists the ZeroDDS call for each
DCPS operation. Samples cross the
C API as CDR bytes encoded by the code `zerodds-idlc generate --lang c`
writes.

## Configuration (`OSPL_URI`)

ZeroDDS does not read the OpenSplice XML configuration. OpenSplice's DDSI2
service is the code base Cyclone DDS grew from, and its networking settings
map as Cyclone's do ([from-cyclonedds.md](from-cyclonedds.md)):

| OpenSplice | `zerodds.yaml` |
|---|---|
| `Domain/Id` | the domain id passed when the participant is created (`zerodds_dpf_create_participant`, `DomainParticipant(domain)`) |
| `DDSI2Service/General/NetworkInterfaceAddress` | `transport.interface` |
| `DDSI2Service/General/AllowMulticast=false` | `discovery.no_multicast: true` |
| `DDSI2Service/Discovery/Peers/Peer@address` | `discovery.peers` |
| `DDSI2Service/Discovery/ParticipantIndex` | none; ZeroDDS takes the first free index |
| `DurabilityService` (TRANSIENT/PERSISTENT) | the in-process durability of the DataWriter, or `zerodds-durability-svc` |

An OpenSplice process creates participants only in the domain its
configuration names in `Domain/Id`; a ZeroDDS participant takes the domain
from the call.

## Mixed deployment

OpenSplice writes and reads XCDR1 only and announces no data representation.
ZeroDDS writers offer XCDR1 first by default
(`wire.data_representation_offer`), so the two match without a setting. A
ZeroDDS reader or writer restricted to XCDR2 does not match an OpenSplice
peer.

OpenSplice's DDSI2 service bundles submessages of several of its own
participants into one datagram with `INFO_SRC` (DDSI-RTPS 2.5 §8.3.7.9);
ZeroDDS reads the receiver state per submessage. Its SPDP announcement
names all participants it hosts in one datagram too, and only one of them
has the SEDP readers a remote writer announces itself to; ZeroDDS reads
every SPDP sample of a datagram (`SpdpReader::samples`).

Measured with the OMG `dds-rtps` suite (105 tests, `-x 1`, OpenSplice CE 6.9
built from source with `tools/opensplice/`; `Test_Domain_0`–`_2` measured
again after the shape application's wrapper learned the domain id):

| Publisher → Subscriber | OK | Not OK |
|---|---|---|
| ZeroDDS → ZeroDDS | 105 | 0 |
| OpenSplice → OpenSplice | 90 | 15 |
| OpenSplice → ZeroDDS | 98 | 7 |
| ZeroDDS → OpenSplice | 80 | 25 |

What the tests that do not pass have in common:

- XCDR2 (`Test_DataRepresentation_1`–`_3`): OpenSplice refuses it.
- Tests OpenSplice does not pass with itself: `Test_Durability_17`,
  `Test_FinalInstanceState_2`, `Test_OrderedAccess_10`/`_17`,
  `Test_CoherentSets_2`/`_5`/`_8`/`_10`/`_12`/`_18`/`_19`/`_21`.
- Coherent sets with TOPIC or GROUP access scope from a foreign writer
  (`Test_CoherentSets_3`/`_4`/`_6`/`_7`/`_11`/`_14`–`_17`/`_20`): an
  OpenSplice reader delivers nothing from a writer that marks its samples
  with `PID_COHERENT_SET`. Cyclone DDS, Fast DDS and OpenDDS as publishers
  give the same result as ZeroDDS; RTI Connext passes because it sends no
  `PID_COHERENT_SET` in these tests.
- A VOLATILE reader behind an OpenSplice writer receives samples written
  before it joined (`Test_Durability_16`); Cyclone DDS, Fast DDS, RTI Connext
  and OpenDDS as subscribers give the same result as ZeroDDS.
- `Test_Ownership_5`/`_6` (two ZeroDDS publishers, one OpenSplice
  subscriber) failed when a ZeroDDS writer matched the OpenSplice reader
  only after OpenSplice's next periodic SPDP announcement, up to 9 s later:
  ZeroDDS read only the first participant of OpenSplice's bundled SPDP
  datagram. Fixed; 8 of 8 runs pass (3 of 8 before). OpenDDS as publisher
  fails `Test_Ownership_5` too.

## Tooling

| OpenSplice | ZeroDDS |
|---|---|
| `idlpp` | `zerodds-idlc` |
| `ospl start` / `ospl stop` | none; there is no daemon to start |
| Tester, `mmstat` | `zerodds-spy`, `zerodds-admin discovery snapshot <domain>` |

## Reading further

- [`tools/opensplice/README.md`](../../tools/opensplice/README.md) — building
  OpenSplice CE 6.9 and its OMG shape application for the measurements above.
- [`docs/wire-axis-flags.md`](../../docs/wire-axis-flags.md) — the
  `opensplice` column.

[opensplice]: https://github.com/ADLINK-IST/opensplice
