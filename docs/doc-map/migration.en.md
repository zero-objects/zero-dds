# Migration to ZeroDDS

German original: [migration.md](migration.md)

This page describes how an existing system is moved from Cyclone DDS, Eclipse OpenSplice, RTI Connext, eProsima Fast DDS or OpenDDS to ZeroDDS: which ZeroDDS API takes the place of the vendor API, whether the IDL passes through `zerodds-idlc` unchanged, what becomes of QoS XML and vendor configuration, how ZeroDDS runs alongside the old vendor on the wire, and which differences remain. The basis is the code on `main`. Statements that come only from a documentation file or a code comment are marked "(per docs)" or "(per code comment)". Statements about the vendor side that are not backed by the repository carry "(vendor side, not checked in the repo)". The wire settings in detail are in [interop-and-dialects.en.md](interop-and-dialects.en.md).

Related pages: [README.en.md](README.en.md) · [architecture.en.md](architecture.en.md) · [crates-core.en.md](crates-core.en.md) · [crates-bridges-services.en.md](crates-bridges-services.en.md) · [crates-languages-codegen.en.md](crates-languages-codegen.en.md) · [config.en.md](config.en.md) · [cli-options.en.md](cli-options.en.md) · [tools-and-man.en.md](tools-and-man.en.md) · [system-setup.en.md](system-setup.en.md) · [security.en.md](security.en.md) · [migration.en.md](migration.en.md) · [interop-and-dialects.en.md](interop-and-dialects.en.md) · [ros2.en.md](ros2.en.md)

## 1 Summary

- **There is no vendor API compatibility layer.** The code contains no `dds_create_participant` (Cyclone C), no `DDS_DomainParticipantFactory_*` (OpenSplice SAC, RTI C), no `DDSDomainParticipantFactory` (RTI legacy C++), no `eprosima::fastdds::dds::*` and no `TheParticipantFactory` (OpenDDS). Hits on these names are found only in vendor test programs under `tools/wire-capture/`, `tools/keyhash-probe/`, `tools/optional-bits/` and in test harnesses.
- **The OMG PSMs are present:** DDS-PSM-Cxx 1.0 under `#include <dds/dds.hpp>` (`crates/cpp/include/dds/`), the OMG DDS Java PSM `org.omg.dds.*` (`crates/java-omgdds/`) and a C API with DCPS structure and its own names `zerodds_*` (`crates/zerodds-c-api/include/zerodds.h`). Whoever uses the ISO C++ API (Cyclone CXX, OpenSplice ISOCPP2, RTI Modern C++) or the OMG Java API today has the shortest path.
- **IDL:** `zerodds-idlc` reads under `Dialect::ALL_VENDORS` by default (`crates/idl-parse/src/dialect.rs:40`). `#pragma keylist` (Cyclone/OpenSplice) and `keylist` (RTI) make members keys. `#pragma DCPS_DATA_KEY` (OpenDDS) makes the named member the key; a path into a member (`p.x`) is rejected (see 2.2).
- **QoS XML:** Only OMG DDS-XML 1.0 is read (`<dds>` or `<qos_library>` as root, `crates/xml/src/parser.rs:81`). RTI `USER_QOS_PROFILES.xml` has this form; an RTI element the reader does not model ends the load with an error that names it (`UnknownElement`, `crates/xml/src/qos_parser.rs`). Fast DDS `<profiles>` is rejected. A loader exists only in Rust (`zerodds_xml::QosProfileRegistry`) and in tools, not in C, C++, Java or Python.
- **Configuration:** No vendor configuration format is read (`CYCLONEDDS_URI`, `OSPL_URI`, `NDDS_QOS_PROFILES`, Fast DDS profiles, OpenDDS `.ini`). ZeroDDS reads `zerodds.yaml` plus drop-ins, `ZERODDS_*` environment variables and `--set` (`crates/dcps/src/runtime.rs:3034`, table `SETTINGS`).
- **Mixed operation:** ZeroDDS uses the standard ports and the standard multicast group from DDSI-RTPS 2.5 §9.6.1.4.1 (`crates/rtps/src/wire_types.rs:1015–1154`). Measured live are Cyclone, Fast DDS, OpenDDS, RTI and OpenSplice CE 6.9 (4.5).

| Vendor | API path in ZeroDDS | IDL unchanged | QoS XML readable | Vendor config readable | Measured live |
|---|---|---|---|---|---|
| Cyclone DDS | C: rewrite on `zerodds_*`; C++: `<dds/dds.hpp>` | yes, mind `--default-extensibility final` | DDS-XML form yes | no | yes (tests `crates/*/tests/cyclone_*`) |
| Eclipse OpenSplice | C (SAC): rewrite; C++ (ISOCPP2): `<dds/dds.hpp>`; Java 5: `org.omg.dds` | `#pragma keylist` yes | not checked | no | yes (OMG `dds-rtps`, 4.5) |
| RTI Connext | C: rewrite; Modern C++: `<dds/dds.hpp>`; Java/C#: rewrite | `keylist`, `//@key` yes; `@RTI_*` ignored | yes, lax | no | yes (`tools/dcps-shapes-interop.sh`, cases E/F) |
| Fast DDS | C++: rewrite (no eProsima API) | yes | no (`<profiles>`) | no | yes (tests `crates/*/tests/fastdds_*`) |
| OpenDDS | C++: rewrite (no TAO IDL mapping API) | yes, `DCPS_DATA_KEY` on members without a path | not applicable | no | yes (cases G/H) |

## 2 Common basics

### 2.1 ZeroDDS bindings that take the place of a vendor API

| Language | Crate | Entry point | API form | Source |
|---|---|---|---|---|
| C | `crates/zerodds-c-api` | `#include <zerodds.h>`, `-lzerodds` | DCPS structure with its own prefixes: `zerodds_dpf_*`, `zerodds_dp_*`, `zerodds_pub_*`, `zerodds_sub_*`, `zerodds_topic_*`, `zerodds_dw_*`, `zerodds_dr_*`; plus the flat runtime API `zerodds_runtime_*`, `zerodds_writer_*`, `zerodds_reader_*` | `include/zerodds.h:4970` (`zerodds_dpf_get_instance`), `:5350` (`zerodds_dp_create_topic`), `:5826` (`zerodds_pub_create_datawriter`), `:5947` (`zerodds_dw_write`) |
| C++ | `crates/cpp` | `#include <dds/dds.hpp>` | DDS-PSM-Cxx 1.0 (`dds::domain`, `dds::topic`, `dds::pub`, `dds::sub`, `dds::core`) over the C API | `include/dds/dds.hpp:1–76` |
| C++ (short) | `crates/cpp` | `#include <zerodds/dds.hpp>` | `zerodds::Runtime`, `Writer`, `Reader`, `TypedWriter<T>` | `include/zerodds/dds.hpp:1–36` |
| Java | `crates/java-omgdds` | `org.omg.dds.core.ServiceEnvironment.createInstance(...)` | OMG DDS Java PSM; own pure Java RTPS stack `org.zerodds.rtps`, no JNI | `java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` |
| Python | `crates/py` | `import zerodds` | `zerodds.DomainParticipantFactory.instance()` … | `crates/py/README.md` |
| C# | `crates/cs` | P/Invoke against `zerodds` | own API | `crates/cs/` |
| TypeScript | `crates/ts-node` | `@zerodds/node` | modelled on DDS-PSM-Cxx (per README) | `crates/ts-node/src/native.ts:154–171` |
| Rust | `crates/dcps`, `crates/rs` | `zerodds_dcps::*` | DCPS entities + `DcpsRuntime` | [crates-core.en.md](crates-core.en.md) |

Properties of the C API that apply to every port:

- Samples cross the boundary as CDR bytes (`zerodds_dw_write(dw, payload, len, handle)`, `zerodds.h:5947`). The encoder is provided by the generated code of the target language (`zerodds-idlc generate --lang c`), not by the library. Cyclone C instead works with a `dds_topic_descriptor_t` that the library serialises itself (vendor side, not checked in the repo).
- The plain `*_create` functions announce `TypeIdentifier::None`; only `zerodds_writer_create_typed`/`zerodds_reader_create_typed` or `zerodds_pub_create_datawriter_typed` pass a serialised COMPLETE `TypeObject` (`crates/zerodds-c-api/README.md`, section "Type model"). Without a TypeObject a peer matches only by topic and type names. OpenDDS requires `use_xtypes=no` for this (comment in `tests/perf/dds-roundtrip-bench/opendds_rtps.ini`).
- Every `*_create` resource is released with the matching `*_destroy`/`*_delete`; `zerodds_buffer_free` releases bytes from `zerodds_reader_take` (header comment `zerodds.h:1–19`).
- Configuration-driven start: `zerodds_runtime_create_from_config(domain_id, config_path, sets, n_sets)` (`zerodds.h:2188`) reads a `zerodds.yaml` and `--set`-style overrides.

Java migration: the implementation class is selected via the system property `org.omg.dds.serviceClassName` or the argument of `ServiceEnvironment.createInstance`; the ZeroDDS class is `org.zerodds.ServiceEnvironmentImpl` (`ServiceEnvironment.java`, class comment and constant `IMPLEMENTATION_CLASS_NAME_PROPERTY`). Code written only against `org.omg.dds.*` stays unchanged when this property changes (per code comment; a run with vendor application code is not documented in the repo).

C++ migration: `crates/cpp/include/dds/` contains `dds.hpp`, `core/*`, `domain/DomainParticipant.hpp`, `topic/{Topic,TopicTraits,ContentFilteredTopic,BuiltinTopic}.hpp`, `pub/{Publisher,DataWriterListener}.hpp`, `sub/{Subscriber,DataReaderListener}.hpp`, `sub/cond/ReadCondition.hpp`, `core/cond/Condition.hpp` (with `WaitSet`). There are no separate headers `dds/pub/DataWriter.hpp`, `dds/sub/DataReader.hpp` and no `QosProvider`. Code that includes these headers directly or uses `dds::core::QosProvider` must be switched to `<dds/dds.hpp>` or written without QosProvider.

### 2.2 IDL

Invocation: `zerodds-idlc check FILE…` checks, `zerodds-idlc generate --lang <language> --out DIR FILE…` generates code. Valid `--lang` values are the names from `Language::as_str` (`crates/lang-cst/src/common.rs:102`): `rust`, `c++`, `c`, `java`, `c#`, `python`, `typescript`, `go`, `ocaml`, `ada`, `d`, `elixir`, `julia`, `lua`, `nim`, `swift`, `zig`, `node`, `ada83`, `f#`, `kotlin`. Further switches: `--strict`, `--deny-warnings`, `--default-extensibility`, `--encoding`, `--endian` (`tools/idlc/src/main.rs:1336–1390`).

Vendor constructs and how the code handles them:

| Construct | Vendor | Behaviour | Source |
|---|---|---|---|
| `#pragma keylist T a b` | Cyclone, OpenSplice | members become keys (`is_key = true`); warning `VendorExtension`, error under `--strict` | `crates/idl-lower/src/lower.rs:668–687`, `crates/idl-lower/src/complete.rs:460–627`; test `a_cyclone_pragma_keylist_reaches_the_same_field_as_rtis_keylist` (`crates/idl-lower/tests/dialects.rs:710`) |
| `keylist T (a, b);` | RTI | as above, vendor flag `Rti` | `crates/idl-parse/src/decl.rs:779–809`; test `an_rti_keylist_reaches_the_vendor_facet_and_the_members` (`dialects.rs:370`) |
| `//@key` | RTI (old), OpenSplice | equivalent to `@key` (XTypes 1.3 §7.3.1.2.5) | `crates/idl-lower/src/annotation.rs:214–220` |
| `#pragma DCPS_DATA_TYPE "T"` | OpenDDS | marks the type in the OpenDDS facet | `lower.rs:689–712`, `complete.rs:309–320` |
| `#pragma DCPS_DATA_KEY T a` | OpenDDS | member becomes key (`is_key = true`), facet `dcps_data_keys` keeps the spelling; an unknown name is reported, a path (`p.x`, `a[0]`) is rejected | `crates/idl-lower/src/complete.rs` (`apply_open_dds_pragmas`); tests `an_opendds_data_key_makes_the_member_a_key`, `an_opendds_data_key_the_type_cannot_take_is_reported` (`dialects.rs`) |
| `@RTI_…`, `@allowed_data_representation(…)` | RTI | unknown annotation, ignored (IDL 4.2 §7.4.15.4.2), arguments are not resolved | `annotation.rs:254–301` |
| `#pragma prefix/version/ID` | CORBA legacy | evaluated (repository IDs) | `lower.rs:645–667` |
| `#pragma dds_xtopics` | XTypes | evaluated and kept | `lower.rs:775–780` |
| every other `#pragma` (e.g. OpenSplice `cats`) | – | kept, warning | `lower.rs:781–813` |

Warnings do not abort `zerodds-idlc`; `--deny-warnings` turns them into errors (`tools/idlc/src/main.rs:370`, `report.rs:44`).

Extensibility without annotation: `zerodds-idlc` assumes `@appendable` by default (help text `main.rs:1349`). Measured: Cyclone uses `final` under XCDR2, RTI/Fast DDS/OpenDDS use `appendable` under XCDR2 (`docs/wire-axis-flags.md`, row `default-extensibility`; result of the evaluation `tools/interop-results/src/dialect_fit.rs`). For IDL written with Cyclone or OpenSplice tools without an extensibility annotation: set `--default-extensibility final` or annotate the types explicitly.

Vendor fixtures that the parser reads: `crates/idl-parse/tests/fixtures/{cyclonedds,fastdds,rti,omg}/` (test `crates/idl-parse/tests/parse.rs`). There are no OpenDDS or OpenSplice fixtures there.

### 2.3 QoS XML

- Parser: `crates/xml` (DDS-XML 1.0, `roxmltree`). Root `<dds>` or `<qos_library>`, otherwise error `expected <dds> or <qos_library> root` (`crates/xml/src/parser.rs:81–99`).
- Profiles: `<qos_library name>` → `<qos_profile name base_name topic_filter>`; `base_name` inheritance with cycle detection (`crates/xml/src/inheritance.rs`, `crates/xml/src/errors.rs:39`); `topic_filter` as attribute and as child element (`qos_parser.rs:283`, `:315–351`).
- Unknown elements in a profile and in entity QoS are an error that names the element (`XmlError::UnknownElement`), as are policies that are no field of the container (DDS 1.4 §2.2.3) and a field given twice. Enumerations take the IDL literals only (`RELIABLE_RELIABILITY_QOS`, not `RELIABLE`). Boolean values are read strictly as `true`/`false`/`1`/`0` (`parse_bool_strict`). `<partition>` is `<name><element>…</element></name>` (§7.2.4.1); octet policies are `<value>` (comma list) or `<valueB64>` (Base64) (§7.2.4.2).
- Use in Rust: `zerodds_xml::QosProfileRegistry::from_file(path)?.writer_qos("Lib::Profile")?` returns `zerodds_qos::WriterQos`; `DataWriterQos::from(...)` takes over all common policies, `data_representation` stays at the default (`crates/xml/src/registry.rs:1–147`, `crates/dcps/src/qos.rs:448–472`). `crates/dcps` does not depend on `zerodds-xml`; the application includes both crates.
- Tools: `zerodds-admin [--skip-unknown] [--vendor-forms] qos validate <file.xml>`, `zerodds-admin qos check <file.xml>` (RxO check), `zerodds-admin config inspect <file.xml>` (`tools/admin/src/main.rs:12–14`); `zerodds-xmlc validate|types|render FILE…` (`tools/xmlc/README.md`).
- No XML loader in the C API, C++ PSM, Java, Python, C#. The Java class `org.omg.dds.core.policy.QosProfile` is an immutable QoS bundle without XML binding.

### 2.4 Configuration

ZeroDDS reads no vendor configuration. The runtime reads (lowest layer first): built-in defaults, `ZERODDS_*` environment variables, `zerodds.yaml` plus `zerodds.d/` (Linux `/etc/zerodds`, macOS `/usr/local/etc/zerodds`, Windows `%PROGRAMDATA%\ZeroDDS`, additionally `/run/zerodds/zerodds.d` on Linux), `--config`/`--set` (`crates/dcps/src/runtime.rs:3013–3024`, `:3233–3247`). An invalid entry in a file prevents startup and reports the location (`runtime.rs:3521–3579`, test `crates/dcps/tests/runtime_config_layering.rs:433–445`). All keys: [config.en.md](config.en.md).

Keys most often needed in a migration:

| Purpose | `zerodds.yaml` | Environment | Default |
|---|---|---|---|
| Unicast peers (without multicast) | `discovery.peers` (list of `ip` or `ip:port`) | `ZERODDS_PEERS` | empty |
| Disable multicast | `discovery.no_multicast` | `ZERODDS_NO_MULTICAST` | `false` |
| SPDP multicast group | `discovery.spdp_multicast_group` | `ZERODDS_SPDP_MC_GROUP` | `239.255.0.1` |
| Participant indices per peer without port | `discovery.max_peer_participants` (1..120) | `ZERODDS_MAX_PEER_PARTICIPANTS` | 10 (`runtime.rs:5121`) |
| Set the network interface | `transport.interface` (IPv4) | `ZERODDS_INTERFACE` | automatic |
| SPDP period | `discovery.spdp_period_ms` | `ZERODDS_SPDP_PERIOD_MS` | 30000 (`runtime.rs:166`); with a lease below 90 s a third of the lease, at least 100 ms (`:5082`) |
| Participant lease | `discovery.participant_lease_duration_ms` | `ZERODDS_PARTICIPANT_LEASE_MS` | see [config.en.md](config.en.md) |
| Maximum message size | `transport.max_message_size` | `ZERODDS_MAX_MESSAGE_SIZE` | see [config.en.md](config.en.md) |
| Data representation of the writer | `wire.data_representation_offer` | `ZERODDS_DATA_REPR_OFFER` | `XCDR1` |
| RTPS dialect | `wire.rtps_dialect` | `ZERODDS_RTPS_DIALECT` | `maximally-interoperable` |
| DDS Security (SROS2 layout) | `security.enclave_dir` | `ZERODDS_SECURITY_DIR` | empty |

The domain ID is passed when the participant is created. The parameters of the port formula are the settings `ports.*` with the defaults `PB=7400`, `DG=250`, `PG=2`, `d0=0`, `d1=10`, `d2=1`, `d3=11` (`crates/rtps/src/wire_types.rs:1015–1154`, read in `crates/dcps/src/runtime.rs:4450–4467`). If a vendor setting sets different ports, both sides need the same values.

### 2.5 Mixed operation, scheme for all vendors

1. **Bring the vendor side to standard wire.** RTPS discovery instead of proprietary discovery, UDPv4 transport next to or instead of shared memory, standard ports, same domain ID. Vendor-specific points are in chapters 3 to 7.
2. **Fix a common IDL.** Both sides generate from the same file. Annotate extensibility explicitly or set `--default-extensibility` accordingly. For constructs whose wire form differs between vendors, generate with `zerodds-idlc generate --dialect <peer>` or avoid them (table in [interop-and-dialects.en.md](interop-and-dialects.en.md), section 6.3).
3. **Transfer the configuration.** Reproduce the vendor configuration in `zerodds.yaml` (tables per vendor), keep QoS profiles as DDS-XML or move them into code.
4. **Add a reading ZeroDDS process.** `zerodds-spy` (`tools/spy`) or `zerodds-admin domain inspect <id>` / `discovery snapshot <id>` (`tools/admin/src/main.rs:7–11`) show whether participants and endpoints are found. The measure is the number of decoded samples, not the match (`docs/interop-harnesses.md`, section "The five stages of a live proof").
5. **Replace one subscriber**, then one publisher, each with the same topic and type name and the same QoS. Adapt `wire.data_representation_offer` to the readers of the other side.
6. **Switch over process by process.** Check both directions after each step.
7. **Remove the vendor installation** when no vendor process runs any more. Then revert vendor-specific settings in `zerodds.yaml` (e.g. `wire.data_representation_offer: [XCDR2]` only for a particular peer).

## 3 Cyclone DDS

### 3.1 API

- Cyclone C (`dds_*` with `dds_entity_t` handles) has no counterpart of the same name. Mapping to the C API with DCPS structure:

| Cyclone C | ZeroDDS C |
|---|---|
| `dds_create_participant(domain, qos, listener)` | `zerodds_dpf_create_participant(zerodds_dpf_get_instance(), domain, qos)` |
| `dds_create_topic(pp, &desc, name, qos, listener)` | `zerodds_dp_create_topic(dp, name, type_name, qos)` or `zerodds_topic_create_typed` |
| `dds_create_publisher` / `dds_create_subscriber` | `zerodds_dp_create_publisher` / `zerodds_dp_create_subscriber` |
| `dds_create_writer` / `dds_create_reader` | `zerodds_pub_create_datawriter[_typed]` / `zerodds_sub_create_datareader[_typed]` |
| `dds_write(wr, &sample)` | `zerodds_dw_write(dw, bytes, len, handle)` with CDR from generated code |
| `dds_take` / `dds_read` | `zerodds_dr_take` / `zerodds_dr_read`, `zerodds_dr_return_loan` |
| `dds_wait_for_acks` | `zerodds_dw_wait_for_acknowledgments` |
| `dds_delete(e)` | `zerodds_dpf_delete_participant`, `zerodds_pub_delete_datawriter` etc. |

- Cyclone CXX (`dds::domain::DomainParticipant` etc.) → `#include <dds/dds.hpp>` from `crates/cpp`. Header layout: see 2.1.
- Shared memory with Cyclone on the same host: `crates/iceoryx-cyclone` and `delivery.mode` (`portable`, `raw-same-host`, `iceoryx`; `runtime.rs:3876`). Details in [crates-bridges-services.en.md](crates-bridges-services.en.md).
- ROS 2: `RMW_IMPLEMENTATION=rmw_zerodds_cpp` instead of `rmw_cyclonedds_cpp` ([ros2.en.md](ros2.en.md)).

### 3.2 IDL

Readable unchanged, `#pragma keylist` becomes keys (2.2). Cyclone idlc assumes `final` without annotation (measurement `docs/wire-axis-flags.md`); ZeroDDS assumes `appendable`. Therefore `zerodds-idlc generate --default-extensibility final …` or explicit `@final`. Under XCDR1, `final` and `appendable` are identical on the wire; the TypeObject still distinguishes them.

### 3.3 QoS

Cyclone QoS in DDS-XML form is read; test `all_four_libraries_roundtrip` (`crates/xml/tests/dds_xml_full.rs:52`).

### 3.4 Configuration (`CYCLONEDDS_URI`)

The Cyclone keys on the left come from `tests/interop/cyclonedds.xml` or the Cyclone documentation (vendor side, not checked in the repo, unless in this file).

| Cyclone | `zerodds.yaml` |
|---|---|
| `General/NetworkInterfaceAddress`, `General/Interfaces/NetworkInterface` | `transport.interface` (one IPv4 address) or `transport.interface_bindings` (file only, `runtime.rs:4089`) |
| `General/AllowMulticast=false` | `discovery.no_multicast: true` |
| `General/MaxMessageSize` | `transport.max_message_size` |
| `Discovery/Peers/Peer@address` | `discovery.peers` |
| `Discovery/MaxAutoParticipantIndex` | `discovery.max_peer_participants` (comment `runtime.rs:5114` names the correspondence) |
| `Discovery/SPDPMulticastAddress` | `discovery.spdp_multicast_group` |
| `Discovery/SPDPInterval` | `discovery.spdp_period_ms` |
| `Discovery/ParticipantIndex` | no key; ZeroDDS takes the first free index 0..119 (`runtime.rs:9148–9165`) |
| `Discovery/Tag` | `discovery.domain_tag` (`ZERODDS_DOMAIN_TAG`), `PID_DOMAIN_TAG` 0x4014 |
| `Discovery/Ports/*` | `ports.port_base`, `ports.domain_gain`, `ports.participant_gain`, `ports.d0` to `ports.d3` (2.4) |
| `Tracing/*` | `diagnostics.*`, `diagnostics.observability` |

### 3.5 Mixed operation

- Standard discovery without additional configuration; evidence `crates/discovery/tests/cyclone_live_sedp.rs`, `crates/dcps/tests/cyclone_live_wlp.rs`, CI gate `tests/interop/ci/` with Cyclone and Fast DDS (`docs/interop-harnesses.md`, table "What each harness covers today").
- A Cyclone reader that accepts only XCDR2 (`shape_main -x 2`) does not match the ZeroDDS default writer; case `N1` in `tools/dcps-shapes-interop.sh:249–255`. Remedy: `wire.data_representation_offer: [XCDR2]`.
- Cyclone sends `PID_KEY_HASH` only to readers that announce `0x801c` (`docs/peer-defects.md`), but reads this vendor-specific PID only from Eclipse and ADLINK participants (`ddsi_plist.c`, 11.0.x). Cyclone does not accept it from ZeroDDS (vendor 01.240); measured against Cyclone 11.0.1: 0 of 167 DATA with `PID_KEY_HASH` (`docs/peer-defects.md`). ZeroDDS does not send `0x801c`; when `PID_KEY_HASH` is missing, the reader computes the KeyHash from its type (`crates/dcps/src/runtime.rs:25692`).

### 3.6 Differences and limits

- With Cyclone, `wstring` code units stay in host byte order; negative union labels below 32 bit select no branch (`docs/peer-defects.md`, two Cyclone sections). ZeroDDS application code follows the specification by default; `zerodds-idlc generate --dialect cyclone` generates Cyclone's form; there is no runtime setting for this ([interop-and-dialects.en.md](interop-and-dialects.en.md), 6.3).
- `xtypes.key_member_bounds`: Cyclone 11.0.1 refuses 3 of 4 OMG `struct_key_*` pairs, like the ZeroDDS default `enforced` (`crates/types/src/assignability.rs:207–227`).

## 4 Eclipse OpenSplice

### 4.1 API

- OpenSplice C (SAC, `DDS_DomainParticipantFactory_get_instance()` etc.): no API of the same name. The ZeroDDS C API has the same DCPS structure (`zerodds_dpf_get_instance`, `zerodds_dpf_create_participant`, `zerodds_dp_create_publisher`, `zerodds_pub_create_datawriter`, `zerodds_dw_write` …); types cross the boundary as CDR bytes.
- ISOCPP2 (`dds::…`) → `#include <dds/dds.hpp>`.
- Java 5 PSM (`org.omg.dds.*`) → `crates/java-omgdds`, system property `org.omg.dds.serviceClassName=org.zerodds.ServiceEnvironmentImpl`. The class comment of `ServiceEnvironment.java` calls the bootstrapping "exactly the RTI Connext / OpenSplice bootstrap idiom" (per code comment).
- Classic OpenSplice C++ (CCPP, `DDS::DomainParticipantFactory::get_instance()`): no equivalent.

### 4.2 IDL

`#pragma keylist` is read (vendor flag `Cyclone`; the comment `crates/zerodds-build/tests/compile.rs:45` names Cyclone, OpenSplice and RTI as users of this notation). `#pragma cats` and other OpenSplice pragmas are kept and produce a warning (2.2). OpenSplice writes XCDR1 only, where `@final` and `@appendable` produce the same bytes; `--default-extensibility` is not needed for an OpenSplice peer. The CDR dialect is `opensplice` (`zerodds-idlc generate --dialect opensplice`); what OpenSplice's IDL cannot express is listed in `documentation/07-migration/from-opensplice.md`.

### 4.3 QoS

The XML format of the OpenSplice QosProvider was not tested against `crates/xml` in the repo. If it is in `<dds>`/`<qos_library>` form, the rules from 2.3 apply.

### 4.4 Configuration (`OSPL_URI`)

`ospl.xml` is not read. At least the following must be reproduced (vendor keys per vendor documentation, not checked in the repo):

| OpenSplice | `zerodds.yaml` |
|---|---|
| `Domain/Id` | domain ID when creating the participant |
| `DDSI2Service/General/NetworkInterfaceAddress` | `transport.interface` |
| `DDSI2Service/General/AllowMulticast` | `discovery.no_multicast` |
| `DDSI2Service/Discovery/Peers` | `discovery.peers` |
| Shared memory domain (`Database`) | no counterpart; same-host paths via `delivery.mode` |

### 4.5 Mixed operation

Measured with the OMG suite `dds-rtps` (105 tests, `-x 1`, OpenSplice CE 6.9 from `tools/opensplice/`): ZeroDDS → ZeroDDS 105, OpenSplice → OpenSplice 90, OpenSplice → ZeroDDS 98, ZeroDDS → OpenSplice 80. The classification of every test that does not pass is in `documentation/07-migration/from-opensplice.md`; none of them is ZeroDDS' any more; `Test_Ownership_5`/`_6` (two ZeroDDS publishers, one OpenSplice subscriber) failed while ZeroDDS read only the first participant of OpenSplice's bundled SPDP datagram, and now pass 8 of 8 runs (3 of 8 before). OpenSplice writes and reads XCDR1 only and announces no data representation; the ZeroDDS default `XCDR1` fits this. OpenSplice bundles submessages of several of its own participants with `INFO_SRC`; ZeroDDS reads the receiver state per submessage (`ParsedDatagram::receiver_walk`, `crates/rtps/src/datagram.rs`).

### 4.6 Differences and limits

- An OpenSplice reader does not deliver coherent sets with TOPIC or GROUP scope from a foreign writer that sets `PID_COHERENT_SET`; measured with ZeroDDS, Cyclone, Fast DDS and OpenDDS as publishers.
- A VOLATILE reader behind an OpenSplice writer receives samples written before it joined (`Test_Durability_16`); measured with ZeroDDS, Cyclone, Fast DDS, RTI and OpenDDS as subscribers.
- `docs/architecture/00_overview.md:79` excludes proprietary wire extensions such as OpenSplice DDSI2E.

## 5 RTI Connext

### 5.1 API

- RTI C (`DDS_*`) and RTI legacy C++ (`DDSDomainParticipantFactory::get_instance()`): no equivalent of the same name; mapping as in 4.1.
- RTI Modern C++ (`dds::…`, `<dds/dds.hpp>`) → `crates/cpp`, same include. RTI-specific namespaces (`rti::…`) have no counterpart.
- RTI Java (`com.rti.dds.*`) → no equivalent; switch to `org.omg.dds.*` in `crates/java-omgdds`.
- RTI tools: `rtiddsgen` → `zerodds-idlc`, `rtiddsspy` → `zerodds-spy`, `rtiddsperf` → `zerodds-perf` (binary names `tools/spy/Cargo.toml`, `tools/perf/Cargo.toml`).

### 5.2 IDL

`keylist`, `//@key`, `@key`, `@nested`, `@topic`, `@default_nested` are read (2.2 and the list of built-in annotations in `crates/idl-lower/src/*.rs`). `@RTI_*` and `@allowed_data_representation` are ignored. Whoever fixed the data representation by annotation sets it in ZeroDDS via `DataWriterQos.data_representation` or `wire.data_representation_offer`.

### 5.3 QoS (`NDDS_QOS_PROFILES`, `USER_QOS_PROFILES.xml`)

- The file has the form `<dds><qos_library><qos_profile>` and is read by `crates/xml`. Example in the repo: `tests/interop/ci/rti/USER_QOS_PROFILES.xml`.
- `is_default_qos="true"` is not evaluated (no hit in `crates/xml/src`). RTI extensions such as `<transport_builtin>`, `<protocol>`, `<resource_limits>` at participant level, `<representation>` end the load with an error that names the element (`qos_parser.rs`). Remove them from the file, or read the file with `zerodds-admin --skip-unknown …` (Rust: `ReadOptions::report_and_skip()`, `QosProfileRegistry::from_file_with`): each element left out appears on stderr with its path. Transfer the policies by hand and run `zerodds-admin qos check`.
- `<participant_qos>` is not a container of DDS-XML 1.0 (its name there is `domain_participant_qos`). The load ends with an error that names the specification's spelling. With `--vendor-forms` (Rust: `ReadOptions::accept_vendor_forms()`) the vendor form is read and reported with its path; the RTI elements inside it stay an error without `--skip-unknown`.
- ZeroDDS does not read the environment variable `NDDS_QOS_PROFILES`.

### 5.4 Configuration

RTI stores discovery and transport settings in `<participant_qos>`. Mapping (vendor keys per vendor documentation, not checked in the repo, except `transport_builtin`):

| RTI | `zerodds.yaml` |
|---|---|
| `<discovery><initial_peers>` / `NDDS_DISCOVERY_PEERS` | `discovery.peers` |
| `<transport_builtin><mask>` | `transport.user_transport(s)` |
| `<discovery><multicast_receive_addresses>` | `discovery.spdp_multicast_group` |
| `<participant_liveliness_lease_duration>` | `discovery.participant_lease_duration_ms` |
| `<property>` `com.rti.serv.load_plugin` (Security) | `security.enclave_dir` or the security API ([security.en.md](security.en.md)) |

### 5.5 Mixed operation

- Evidence via `tools/dcps-shapes-interop.sh` (cases E, F: classic `@final` ShapeType against `ShapeType_publisher/subscriber`) and `zerodds-examples/cross-vendor-shapes/` (per `docs/interop-harnesses.md`, table "What each harness covers today"). RTI is not part of the CI gate `tests/interop/ci/`.
- RTI prefers shared memory on the same host; without `transport_builtin` = `UDPv4` a foreign stack sees the participant, matches and receives zero samples (comment in `tests/interop/ci/rti/USER_QOS_PROFILES.xml`).
- The RTI DataReader accepts only XCDR1 by default (same comment). The ZeroDDS default `XCDR1` fits; a ZeroDDS writer with `[XCDR2]` needs `XCDR2_DATA_REPRESENTATION` in the reader on the RTI side.
- GROUP-coherent sets ZeroDDS → Connext 7.7.0: `Test_CoherentSets_21` passes, `_12` does not (3 runs each).
- Without `NDDSHOME`, RTI starts no participant (trap 1 in the header of `tools/dcps-shapes-interop.sh`).

### 5.6 Differences and limits

- `wstring` and `wchar` under XCDR1: RTI counts characters, writes a terminator and uses 4-byte units (`docs/wire-axis-flags.md`, column `RTI¹`). ZeroDDS application code writes the specification form. For these types there is no runtime setting; avoid affected members or use XCDR2.
- RTI security is not measured (licence; `docs/spec-coverage/cross-vendor-validation.md`, footnote 2).
- Routing Service, Cloud Discovery Service, Persistence Service: separate ZeroDDS services with their own configuration ([crates-bridges-services.en.md](crates-bridges-services.en.md)); no RTI configuration format.

## 6 eProsima Fast DDS

### 6.1 API

The Fast DDS C++ API (`eprosima::fastdds::dds::DomainParticipantFactory::get_instance()->create_participant(...)`) has no counterpart. The target is `<dds/dds.hpp>` (DDS-PSM-Cxx) or `<zerodds/dds.hpp>`. The DCPS names (DomainParticipant, Publisher, DataWriter, …) stay; namespace, creation (constructors instead of `create_*` with raw pointers) and type support (`TopicDataType`/`TypeSupport` → generated `topic_type_support<T>`) change. `fastddsgen` → `zerodds-idlc generate --lang c++`.

### 6.2 IDL

Readable unchanged; fixtures `crates/idl-parse/tests/fixtures/fastdds/`. Fast DDS ignores `@bit_bound` on enums and sorts map entries by key (`docs/wire-axis-flags.md`, rows `enum-holder`, `map-entries`). Convert IDL that relies on this to 32-bit enums and without `map` if both sides need the same bytes.

### 6.3 QoS

Fast DDS XML (`<profiles>`) is not read: root `<profiles>` yields `InvalidXml`, `<dds><profiles>` yields zero libraries (`crates/xml/src/parser.rs:81–99`). Transfer profiles into code or rewrite them as DDS-XML.

### 6.4 Configuration (`FASTDDS_DEFAULT_PROFILES_FILE`, `DEFAULT_FASTDDS_PROFILES.xml`)

Not read. Mapping (vendor keys per vendor documentation, not checked in the repo):

| Fast DDS | `zerodds.yaml` |
|---|---|
| `<initialPeersList>` | `discovery.peers` |
| `<metatrafficUnicastLocatorList>`, `<interfaceWhiteList>` | `transport.interface`, `transport.interface_bindings` |
| `<useBuiltinTransports>false` + UDPv4 transport | `transport.user_transport: UDPv4` |
| Discovery Server client (`<discoveryProtocol>CLIENT`) | no client for the Fast DDS Discovery Server protocol found; ZeroDDS has its own SPDP relay (`discovery.server`, `runtime.rs:1858–1868`) |
| `<leaseDuration>` | `discovery.participant_lease_duration_ms` |

### 6.5 Mixed operation

- Standard discovery; evidence `crates/discovery/tests/fastdds_live_spdp.rs`, `crates/dcps/tests/fastdds_live_pub.rs`, `fastdds_live_sub.rs`, `fastdds_qos_matrix.rs`, CI gate `tests/interop/ci/`. An SPDP handshake with a running Fast DDS Discovery Server exists as an `#[ignore]` test (`fastdds_live_spdp.rs:131`).
- ROS 2 services with `rmw_fastrtps`: ZeroDDS writes `PID_RELATED_SAMPLE_IDENTITY` twice, as 0x0083 and as eProsima PID 0x800f (`crates/dcps/src/runtime.rs:17939–17947`, `crates/rtps/src/inline_qos.rs:119–127`). No setting needed.
- DDS Security with Fast DDS needs RSA identity certificates (per docs, `docs/spec-coverage/cross-vendor-validation.md`, section "DDS-Security, cross-vendor").

### 6.6 Differences and limits

- `wstring` counts characters (`docs/wire-axis-flags.md`, row `wstring-counts`); no runtime setting in ZeroDDS.
- Fast DDS writes `@final` and `@appendable` as XCDR1 and `@mutable` as PL_CDR1 (per docs, `cross-vendor-validation.md`, table of encapsulation IDs). The ZeroDDS reader decodes according to the encapsulation header (`crates/dcps/src/subscriber.rs:56–79`).

## 7 OpenDDS

### 7.1 API

OpenDDS uses TAO's IDL-to-C++ mapping (`DDS::DomainParticipantFactory_var dpf = TheParticipantFactoryWithArgs(argc, argv)`, `_var` types, `TypeSupport` classes, `-DCPSConfigFile`). None of this exists in ZeroDDS. The target is `<dds/dds.hpp>` or the C API. The migration affects entity creation, memory management (`_var` → RAII) and type support.

### 7.2 IDL

`#pragma DCPS_DATA_TYPE` and `#pragma DCPS_DATA_KEY` are read; `DCPS_DATA_KEY` makes the member the key (2.2). A key path into a member (`p.x`, `a[0]`) is rejected and must be written as `@key` on the inner type. OpenDDS ignores `@value` on enumerators and `@bit_bound` (`docs/wire-axes-followup.md`, table "What moved into a dialect", rows `EnumeratorValue::Ordinal`, `EnumHolderWidth`).

### 7.3 QoS

OpenDDS has no XML QoS profile in the repo context; QoS lives in the application code and is transferred there.

### 7.4 Configuration (`-DCPSConfigFile *.ini`)

Example in the repo: `tests/perf/dds-roundtrip-bench/opendds_rtps.ini`.

| OpenDDS | `zerodds.yaml` or note |
|---|---|
| `[common] DCPSDefaultDiscovery=rtps_disc` | required on the OpenDDS side; the OpenDDS default is InfoRepo (comment in the `.ini`) |
| `[transport/…] transport_type=rtps_udp` | required on the OpenDDS side |
| `[rtps_discovery/…] use_xtypes=no` | needed when ZeroDDS writers are created without a TypeObject (plain `*_create`); not needed with `*_typed` |
| `[rtps_discovery/…] SpdpSendAddrs`, static peers | `discovery.peers` |
| `ResendPeriod`, `LeaseDuration` | `discovery.spdp_period_ms`, `discovery.participant_lease_duration_ms` |

### 7.5 Mixed operation

- Cases G, H in `tools/dcps-shapes-interop.sh` (classic ShapeType, `-DCPSConfigFile rtps.ini`), M15/M16 (OpenDDS writer writes XCDR2 `0x0007`, 194 samples, per comment in the script). OpenDDS is not part of the CI gate.
- An OpenDDS reader that accepts only XCDR2 needs `wire.data_representation_offer: [XCDR2]`: the writer's first element has to be in the reader's list (`negotiate`, default `strict`, `crates/rtps/src/publication_data.rs:166–173`); which list the OpenDDS reader announces is not measured individually in the repo).
- Strict encapsulation check on the OpenDDS side (`EncapsulationHeader::to_encoding`): ZeroDDS sets the header according to the declared extensibility (`crates/dcps/src/publisher.rs:3585–3603`).
- Secure SPDP against OpenDDS: the default `SpdpResponseTargets::AnnouncedLocatorsAndBeaconSource` is needed for OpenDDS (`crates/rtps/src/dialect.rs`, section `SpdpResponseTargets`; measured 0/5 versus 5/5). `wire.rtps_dialect: cyclone` or `fastdds` switches this value off.

### 7.6 Differences and limits

- OpenDDS security: in the matrix `docs/spec-coverage/cross-vendor-validation.md`, section "DDS-Security, cross-vendor", the simple type does not match when secured (`NO_MATCH`/`FAIL`), the rich type matches in both directions (194 µs and 125 µs p50 respectively) (per docs). ZeroDDS writes the algorithm names in the handshake by default (`security.algo_name_nul: spec`) with the terminating NUL that OpenDDS requires (`crates/security/src/vendor_forms.rs:10–19`, `crates/discovery/src/security/stack.rs:1098–1103`).
- Enumerator values, union framing and `@mutable` member headers under XCDR1 differ (`docs/wire-axis-flags.md`, columns `ODDS¹`, `ODDS²`); no runtime setting.

## 8 Open and contradictory points

| Point | State in the code | Contradiction or gap |
|---|---|---|
| OpenSplice | no measurement, no harness | all OpenSplice statements on this page are without evidence |
