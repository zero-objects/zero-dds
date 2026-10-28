# 07 – Migration Guides

Moving an application from another DDS implementation to ZeroDDS. There is
no vendor-API compatibility layer: code written against `dds_*` (Cyclone C),
`eprosima::fastdds::dds::*`, RTI's `DDS_*`/`rti::*` or OpenDDS's TAO mapping
is rewritten. What carries over unchanged is the wire (DDSI-RTPS 2.5), the IDL,
QoS written as OMG DDS-XML, and code written against the OMG PSMs.

## Available migration guides

| From | Guide |
|---|---|
| Eclipse Cyclone DDS | [from-cyclonedds.md](from-cyclonedds.md) |
| eProsima Fast DDS | [from-fastdds.md](from-fastdds.md) |
| RTI Connext DDS | [from-rti-connext.md](from-rti-connext.md) |
| OpenDDS | [from-opendds.md](from-opendds.md) |
| Eclipse OpenSplice | [from-opensplice.md](from-opensplice.md) |

## Where your code lands

| Your API | ZeroDDS |
|---|---|
| ISO C++ / DDS-PSM-Cxx (Cyclone-CXX, RTI Modern C++, OpenSplice ISOCPP2) | `#include <dds/dds.hpp>` from `crates/cpp` — the shortest path |
| OMG Java PSM (`org.omg.dds.*`) | `crates/java-omgdds`, a pure-Java stack; select it with `org.omg.dds.serviceClassName` |
| A vendor C API | `crates/zerodds-c-api`, `zerodds.h`: DCPS structure with `zerodds_dpf_*`, `zerodds_dp_*`, `zerodds_pub_*`, `zerodds_sub_*`, `zerodds_dw_*`, `zerodds_dr_*` |
| Vendor C++ (Fast DDS, OpenDDS, RTI legacy) | `<dds/dds.hpp>` or the short `<zerodds/dds.hpp>` (`zerodds::Runtime`) |
| Python, C#, TypeScript, Rust | `crates/py`, `crates/cs`, `crates/ts-node`, `crates/dcps` |

Samples cross the C API as CDR bytes; the encoder is the code `zerodds-idlc
generate --lang <language>` writes, not the library.

## What carries over

- **IDL.** `zerodds-idlc` reads the vendors' extensions by default:
  `#pragma keylist` (Cyclone, OpenSplice), `keylist` and `//@key` (RTI) and
  `#pragma DCPS_DATA_KEY` (OpenDDS) make members keys. Unknown annotations such
  as `@RTI_*` are ignored; unknown pragmas are kept and reported as warnings.
  An aggregate without an extensibility annotation is `appendable`; Cyclone's
  `idlc` makes it `final` — pass `--default-extensibility final` for IDL written
  for Cyclone.
- **QoS as OMG DDS-XML.** `zerodds_xml::QosProfileRegistry` (Rust) reads
  `<dds><qos_library><qos_profile>`, with `base_name` inheritance; RTI's
  `USER_QOS_PROFILES.xml` has this form. An element the reader does not model
  (a vendor policy, `<representation>`, a policy in a container that does not
  have it in DDS 1.4) ends the load with an error that names the element;
  remove it, or read the file with `zerodds-admin --skip-unknown …` (the
  Rust loaders have `QosProfileRegistry::from_file_with` and
  `ReadOptions::report_and_skip()`), which leaves each such element out and
  lists it with its path; carry those policies over by hand. Spellings other
  products use for something the schemas define (`<participant_qos>`,
  `<topic_filter>` as an element, IDL type names) are refused with the
  DDS-XML 1.0 form; `--vendor-forms` / `ReadOptions::accept_vendor_forms()`
  reads them and reports each. Check the result with
  `zerodds-admin qos check <file.xml>`. There
  is no XML loader in the C, C++, Java or Python bindings, and Fast DDS's
  `<profiles>` format is not read.
- **The wire.** Standard ports and the standard multicast group (DDSI-RTPS 2.5
  §9.6.1.4.1). Measured live against Cyclone DDS, Fast DDS, OpenDDS, RTI
  Connext and OpenSplice.

## What does not

- **Vendor configuration.** `CYCLONEDDS_URI`, `NDDS_QOS_PROFILES`, Fast DDS
  profile files and OpenDDS `.ini` files are not read. ZeroDDS reads
  `zerodds.yaml` with its drop-ins, `ZERODDS_*` variables and `--set`
  ([zerodds.yaml(5)](../../man/man5/zerodds.yaml.5)); each guide maps the common
  vendor settings.
- **Ports.** The port formula is fixed; a vendor setting that moves ports has to
  go back to the standard for a mixed deployment.

## Running both side by side

1. Put the vendor side on the standard wire: RTPS discovery, UDPv4 next to or
   instead of shared memory, standard ports, the same domain id.
2. Generate both sides from one IDL file, with the extensibility stated.
3. Carry the configuration over into `zerodds.yaml`.
4. Add a reading ZeroDDS process: `zerodds-spy -d <domain> -t <topic>` prints
   what it receives. Count decoded samples; a match alone proves nothing.
5. Replace one subscriber, then one publisher, with the same topic, type and
   QoS; check both directions after each step.
6. Remove the vendor installation once no vendor process runs.

## Common caveats

- **Multicast.** Where the network filters multicast, list the peers:
  `discovery.peers` and `discovery.no_multicast: true` — each vendor has the
  same setting under its own name.
- **Data representation.** A ZeroDDS writer offers `[XCDR1, XCDR2]` and
  writes XCDR1. A reader that accepts only XCDR2 needs
  `wire.data_representation_offer: [XCDR2]` on the writer side.
- **Security.** Governance and permissions documents are S/MIME-signed by the
  Permissions CA; `zerodds-secure-permissions` signs them
  ([`tools/secure-permissions`](../../tools/secure-permissions/README.md)).

## Reading further

- [Spec coverage](../../docs/spec-coverage/) — which spec sections ZeroDDS
  implements.
- [Integration — Java](../05-integration/java.md) /
  [TypeScript](../05-integration/typescript-wasm.md).
