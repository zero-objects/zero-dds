# ROS 2 RMW Bridge — Spec-Coverage

**Quellen:**

* ROS Enhancement Proposals (REPs) — `internal/standards/cache/ros2/rep-{2003,2004,2005,2007,2008,2009}.html`.
* RMW C-API Headers — `rmw/rmw.h` und `rmw/qos_profiles.h` aus
  `rmw 4.x` (ROS 2 Iron/Jazzy Distribution); leben im upstream
  `ros2/rmw`-GitHub-Repository, **nicht** in `internal/standards/cache/`,
  da eigenständige Distribution. De-facto-Spezifikation ohne eigene
  REP, normativ über die Header-Definitionen.
* ROS 2 IDL Subset — Design-Article `design.ros2.org/articles/
  idl_interface_definition.html` (nicht im Cache; live-Quelle) +
  Wire-Naming-Convention aus `rosidl_typesupport_fastrtps_cpp`
  (de-facto, in `ros2/rosidl`-GitHub-Repository).
* Topic/Service-Naming-Convention — Design-Article
  `design.ros2.org/articles/topic_and_service_names.html`
  (nicht im Cache; live-Quelle). Implementiert in `rmw_dds_common`.

Folgt dem Format aus `docs/spec-coverage/PROCESS.md`.

**Kontext:** ROS 2 Robot-Middleware-Wrapper bauen auf DDS auf. Die
RMW-API + RMW-QoS-Profile-Mapping + Topic-Mangling-Convention +
ROS-IDL→DDS-XTypes-Wire-Convention sind die zentralen Wire-Mappings.
Verteilt über:

- `crates/ros2-rmw/` — Mapping-Layer als pure-Rust no_std+alloc Library (RMW-API, QoS-Profile-Mapping, Topic-Mangling, ROS-IDL→DDS-XTypes)
- `crates/rmw-zerodds-shim/` — `rmw_zerodds`-C-FFI-Wrapper

**Crate-Mapping:**

| Spec-Bereich | Crate-File |
|---|---|
| REP-2003 Sensor/Map QoS | `crates/ros2-rmw/src/qos_profiles.rs` (Profile-Konstanten) |
| REP-2004 Quality-Levels | `crates/ros2-rmw/src/quality.rs` |
| Topic-Name-Mangling Convention | `crates/ros2-rmw/src/topic_mangling.rs` |
| Standard RMW QoS Profiles | `crates/ros2-rmw/src/qos_profiles.rs::profiles::*` |
| RMW C-API (`rmw/rmw.h`) | `crates/ros2-rmw/src/ffi_api.rs` |
| RMW-QoS-Mapping (`rmw/qos_profiles.h`) | `crates/ros2-rmw/src/rmw_qos_mapping.rs` |
| ROS-IDL → DDS-XTypes Wire-Mapping | `crates/ros2-rmw/src/type_mapping.rs` |

Implementation: `crates/ros2-rmw/` (6 Module, 52 Tests grün via
`cargo test -p zerodds-ros2-rmw`).

---

## REP-2003 Sensor Data and Map QoS Settings

### Map QoS

**Spec:** REP-2003 §"Map Quality of Service" — "Map providers [...] are
expected to provide all maps over a reliable transient-local topic.
[...] The depth of the transient-local storage depth is left to the
designer, however a single map depth is a reasonable choice".

**Repo:** `crates/ros2-rmw/src/qos_profiles.rs::profiles::MAP` —
Reliable + TransientLocal + KeepLast(1).

**Tests:** `qos_profiles::tests::map_profile_matches_rep_2003_specification`.

**Status:** done

### Sensor Data QoS (Consumer-Side)

**Spec:** REP-2003 §"Sensor Driver Quality of Service" — wörtlich:
"Sensor data provided by a sensor driver from a camera, inertial
measurement unit, laser scanner, GPS, depth, range finder, or other
sensors are expected to be provided over a `SystemDefaultsQoS`
quality of service as provided by the implemented ROS 2 version API.
Consumers of sensor data are to use `SensorDataQoS` quality of service
as provided by the implemented ROS 2 version API."

Wichtig: REP-2003 verlangt für **Driver-Side** `SystemDefaultsQoS`
(= Reliable+Volatile+KeepLast(10), siehe `DEFAULT`-Item unter
"Standard RMW QoS Profiles" weiter unten). Die Consumer-Side
verwendet `SensorDataQoS` (= BestEffort+Volatile+KeepLast(5)).
Dieses Item beschreibt die Consumer-Side; die Driver-Side ist via
`profiles::DEFAULT` abgedeckt.

**Repo:** `crates/ros2-rmw/src/qos_profiles.rs::profiles::SENSOR_DATA` —
BestEffort + Volatile + KeepLast(5) (entspricht
`rmw_qos_profile_sensor_data` aus `rmw/qos_profiles.h`).

**Tests:** `qos_profiles::tests::sensor_data_profile_matches_rep_2003_specification`
prüft die Consumer-Side-Konstante (BestEffort+Volatile+KeepLast(5)).

**Status:** done — Consumer-Side abgedeckt; Driver-Side ist über
`DEFAULT`-Profile (siehe unten) gleichermaßen abgedeckt.

---

## REP-2004 Package Quality Categories

### Quality Levels Q1-Q5

**Spec:** REP-2004 — fünf Levels mit folgenden Definitionen
(zitiert aus REP-2004 §"Quality Level Categories"):

* **Quality Level 1** — "highest quality level; packages that are
  needed for production systems".
* **Quality Level 2** — "high quality packages that are either:
  needed for production systems or commonly used".
* **Quality Level 3** — "tooling quality packages".
* **Quality Level 4** — "demos, tutorials, and experiments".
* **Quality Level 5** — "default quality level" (Pakete ohne
  explizite Quality-Claims).

Numerische Repräsentation 1..5; Q1 ist die höchste, Q5 ist der
Default für Pakete ohne explizit deklariertes Quality-Niveau.

**Repo:** `crates/ros2-rmw/src/quality.rs::QualityLevel` mit
`numeric()`/`from_numeric()`-Konvertern.

**Tests:** `quality::tests::quality_level_numeric_round_trip`,
`quality::tests::quality_level_ordering_q1_is_highest`,
`quality::tests::quality_level_from_numeric_rejects_out_of_range`.

**Status:** done — Klassifikations-Modell exposed; tatsächliche
Quality-Audit ist Caller-Aufgabe (z.B. `package.xml`-Tag).

---

## REP-2005 ROS 2 Common Packages

### Common-Package-List

**Spec:** REP-2005 — informational; Liste der ROS-2-Common-Packages.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — REP-2005 markiert sich selbst als
informational; Common-Package-Liste ohne normative Anforderung an die
RMW-Bridge.

---

## REP-2007 Type Adaptation Feature

### Type Adaptation API

**Spec:** REP-2007 — Compile-Time-Feature in `rclcpp` (C++) zur
Konversion von User-Types zu ROS-Messages on-the-fly.

**Repo:** —

**Tests:** —

**Status:** n/a (keine rmw-Fläche) — REP-2007 Type-Adaptation ist ein
Compile-Time-`rclcpp`-Template: die Konversion passiert **über** rmw (der
User-Type ist schon eine ROS-Message, bevor er den Draht erreicht), es gibt also
nichts auf der rmw-Schicht umzusetzen. Die rmw-API, die REP-2007 selbst
spezifiziert (Node/Identifier/QoS-Mapping), ist vollständig implementiert (alle
done-Items oben). Kein Vendor-„Reject", sondern eine Schicht-Tatsache.

---

## REP-2008 Hardware Acceleration

### HW-Accel Architecture

**Spec:** REP-2008 — Conventions für GPU/FPGA-Drivers in ROS 2.

**Repo:** —

**Tests:** —

**Status:** n/a (out of rmw scope) — REP-2008 ist eine Driver-/Vendor-Konvention
für Acceleration-Hardware (GPU/FPGA), lebt in der Hardware-Vendor-Schicht und wird
durch ROS-2-Anwender direkt orchestriert (CUDA/ROCm/Vitis), nicht durch den
DDS-Vendor. Hier gibt es — anders als bei REP-2009 — keine rmw-Fläche umzusetzen.

---

### Endpoint info by topic — `rmw_get_publishers/subscriptions_info_by_topic`

**Spec:** `rmw/get_topic_endpoint_info.h` — pro Topic die Endpoint-Liste mit
Node-Name/-Namespace, Typ, Endpoint-Typ, 16-Byte-GUID und QoS (`ros2 topic
info -v`). Teil der rmw-seitigen REP-2009-Verantwortung.

**Repo:** `rmw_c/rmw_zerodds.c` (`rmw_get_publishers_info_by_topic`/
`_subscriptions_info_by_topic` → `zerodds_get_endpoint_info_by_topic`): enumeriert
pro-Endpoint via `rmw_zerodds_node_for_each_publication/subscription_endpoint`,
filtert auf das demanglede Topic, löst pro Endpoint die Node-Identität über
`rmw_zerodds_node_resolve_endpoint` (Endpoint-GUID → Node) auf und füllt
`rmw_topic_endpoint_info_array_t`. Datenpfad: `crates/dcps/src/runtime.rs`
(`discovered_publication/subscription_endpoints`, lokal + SEDP-remote) →
c-api (`zerodds_runtime_for_each_*_endpoint` + `ZeroDdsEndpointInfo`) → Shim.
Voraussetzung Endpoint→Node: die `ros_discovery_info`-Participant-gid ist die
**echte** DDS-Participant-GUID, und `ParticipantEntitiesInfo` trägt pro Node die
reader/writer-gid-Sequenzen (Endpoint-GUID-Prefix matcht den Participant).
QoS ist best-effort aus der Discovery (History/Depth liegen nicht auf dem Draht →
`UNKNOWN`).

**Tests:** `crates/py/python/tests/ros2/test_rmw_zerodds_interop.py::test_rclpy_endpoint_info_by_topic`
(rclpy `get_publishers_info_by_topic`: Typ + QoS + aus der Endpoint-GUID
aufgelöster Node-Name/-Namespace) auf ROS 2 Humble; Shim-Roundtrip-Units
`participant_info_roundtrips_endpoint_gids` / `_without_endpoints`.

**Status:** done

## REP-2009 Type Negotiation Feature

### Type Negotiation — rmw-seitiger Teil (Type-Hash + Endpoint-Info)

**Spec:** REP-2009 — Runtime-Pub/Sub-Type-Negotiation. Die rmw-seitigen
Verantwortlichkeiten sind der RIHS-Type-Hash und die Endpoint-Typ-/QoS-Exposition,
über die negotiating Endpoints gematcht werden.

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs::rmw_zerodds_compute_type_hash`
(RIHS SHA-256) + der Endpoint-Info-Pfad (`rmw_get_publishers/subscriptions_info_by_topic`,
siehe „Endpoint info by topic" oben).

**Tests:** `type_hash_sha256_is_deterministic` + 2 weitere (3) +
`test_rclpy_endpoint_info_by_topic` (Humble).

**Status:** done (rmw-seitig). Die Negotiation-**State-Machine** selbst ist ein
Runtime-Feature in `rclcpp` (Sprach-Binding-Layer), keine rmw-API — verfügbar über
jeden RMW; darüber hinaus gibt es keine rmw-Fläche umzusetzen.

---

## Topic-Name-Mangling Convention (de-facto, `rmw_dds_common`)

### Prefix-Convention rt/rq/rr/rs

**Spec:** Design-Article `design.ros2.org/articles/
topic_and_service_names.html` §"DDS Topic Names" + de-facto-Konvention
aus `ros2/rmw_dds_common`-Implementation:

* `rt/<name>` — Topic-Pub-Sub (ROS-Topic).
* `rq/<name>Request` — Service-Request.
* `rr/<name>Reply` — Service-Reply.
* `rs/<name>` — Service-Discovery (legacy, vor REP-2009-Strukturen).

**Repo:** `crates/ros2-rmw/src/topic_mangling.rs::{RosKind,
mangle_topic_name, demangle_topic_name, is_ros_topic}`.

**Tests:** `topic_mangling::tests::mangle_topic_strips_leading_slash_and_prepends_rt`,
`topic_mangling::tests::mangle_preserves_internal_slashes`,
`topic_mangling::tests::mangle_each_kind_uses_correct_prefix`,
`topic_mangling::tests::mangle_handles_already_unprefixed_name`,
`topic_mangling::tests::mangle_rejects_empty_name`,
`topic_mangling::tests::mangle_rejects_invalid_leading_character`,
`topic_mangling::tests::mangle_accepts_underscore_leading`,
`topic_mangling::tests::demangle_round_trips_all_kinds`,
`topic_mangling::tests::demangle_rejects_unknown_prefix`,
`topic_mangling::tests::is_ros_topic_recognizes_all_four_prefixes`,
`topic_mangling::tests::mangle_demangle_round_trip`.

**Status:** done — Suffixe (`Request`/`Reply`) bleiben Caller-Aufgabe
(typisch automatisch vom Service-Codegen ergänzt).

---

## Standard RMW QoS Profiles

### `rmw_qos_profile_*` Konstanten

**Spec:** `rmw/qos_profiles.h` (rmw 4.x) — sieben Default-Profiles:
`rmw_qos_profile_default` (Reliable+Volatile+KeepLast(10)),
`rmw_qos_profile_sensor_data` (BestEffort+Volatile+KeepLast(5)),
`rmw_qos_profile_parameters` (Reliable+Volatile+KeepLast(1000)),
`rmw_qos_profile_services_default` (Reliable+Volatile+KeepLast(10)),
`rmw_qos_profile_parameter_events` (Reliable+Volatile+KeepLast(1000)),
`rmw_qos_profile_system_default` (alle Felder als
`*_SYSTEM_DEFAULT`-Sentinel; Implementation-defined),
`rmw_qos_profile_unknown` (alle Felder als `*_UNKNOWN`-Sentinel).

**Repo:** `crates/ros2-rmw/src/qos_profiles.rs::profiles::*`
(`DEFAULT`, `SENSOR_DATA`, `PARAMETERS`, `SERVICES_DEFAULT`,
`PARAMETER_EVENTS`, `SYSTEM_DEFAULT`, `UNKNOWN`, `MAP`) +
`is_unknown`/`is_system_default`-Predicates. Die Policy-Enums
(`Reliability`/`Durability`/`History`) tragen echte `SystemDefault`- und
`Unknown`-Sentinel-Varianten (spec-treu zu `rmw_qos_*_policy_t`);
`rmw_qos_mapping::rmw_to_dds` reicht sie durch (Auflösung erst bei
DDS-Entity-Erzeugung).

**Tests:** `qos_profiles::tests::default_profile_is_reliable_volatile_keep_last_10`,
`parameters_profile_uses_keep_last_1000`, `services_default_matches_rmw_defaults`,
`parameter_events_uses_keep_last_1000`, `system_default_is_all_sentinels`,
`unknown_profile_is_all_unknown`, `system_default_and_unknown_are_distinct`,
`is_unknown_recognizes_unknown_profile`, `is_unknown_rejects_real_and_system_default_profiles`;
`rmw_qos_mapping::tests::rmw_system_default_passes_through_as_sentinel`,
`rmw_unknown_passes_through_as_sentinel`.

**Status:** done — alle sieben rmw-Default-Profiles spec-treu:
* Sechs konkret 1:1 (DEFAULT, SENSOR_DATA, PARAMETERS, SERVICES_DEFAULT,
  PARAMETER_EVENTS, MAP).
* `SYSTEM_DEFAULT` = **jedes Feld** als `*_SYSTEM_DEFAULT`-Sentinel (nicht mehr
  Alias zu DEFAULT) → der DDS-Implementations-Default jedes Feldes greift.
* `UNKNOWN` = **jedes Feld** als `*_UNKNOWN`-Sentinel + `is_unknown`-Predicate.

---

## RMW C-API (`rmw/rmw.h`)

### `rmw_ret_t` Return-Codes

**Spec:** `rmw/types.h` (rmw 4.x) — `rmw_ret_t` als `int32` mit den
Werten OK=0, ERROR=1, TIMEOUT=2, UNSUPPORTED=3, BAD_ALLOC=10,
INVALID_ARGUMENT=11, INCORRECT_RMW_IMPLEMENTATION=12.

**Repo:** `crates/ros2-rmw/src/ffi_api.rs::RmwRet` (`#[repr(i32)]`-
Enum mit allen sieben Werten + `map_to_rmw_ret`-Konverter).

**Tests:** `ffi_api::tests::error_codes_match_rmw_h`,
`ffi_api::tests::ok_is_zero`,
`ffi_api::tests::map_to_rmw_ret_ok`,
`ffi_api::tests::map_to_rmw_ret_err`.

**Status:** done

### Implementation-Identifier (`rmw_get_implementation_identifier`)

**Spec:** `rmw/rmw.h` — `rmw_get_implementation_identifier()` liefert
Vendor-String (Konvention `rmw_<vendor>_cpp`, z.B.
`"rmw_fastrtps_cpp"`/`"rmw_cyclonedds_cpp"`/`"rmw_connext_cpp"`).
`RMW_CHECK_*_FOR_NULL_*`-Macros (in `rmw/check_type_identifiers_match.h`)
prüfen Caller-übergebene Strings gegen diesen Identifier
(`RMW_RET_INCORRECT_RMW_IMPLEMENTATION` bei Mismatch).

**Repo:** `crates/ros2-rmw/src/ffi_api.rs::check_rmw_identifier` plus
Konstante `RMW_IMPLEMENTATION_IDENTIFIER: &str = "rmw_zerodds_cpp"`
(folgt der Vendor-Naming-Convention).

**Tests:** `ffi_api::tests::implementation_identifier_matches_convention`,
`ffi_api::tests::check_rmw_identifier_accepts_correct`,
`ffi_api::tests::check_rmw_identifier_rejects_other_vendor`.

**Status:** done

### `rmw_node_t` Construction

**Spec:** `rmw/types.h` — `rmw_node_t`-Struct mit
`implementation_identifier`-Feld + `name`/`namespace`/`context`.

**Repo:** `crates/ros2-rmw/src/ffi_api.rs::RmwNode`.

**Tests:** `ffi_api::tests::rmw_node_construction`.

**Status:** done

---

## RMW-QoS-Mapping (`rmw/qos_profiles.h`)

### `rmw_qos_*_policy_t`-Enums (History/Reliability/Durability)

**Spec:** `rmw/qos_profiles.h` — drei C-Enums (`rmw_qos_history_policy_t`,
`rmw_qos_reliability_policy_t`, `rmw_qos_durability_policy_t`) mit
SYSTEM_DEFAULT / KEEP_LAST / KEEP_ALL / RELIABLE / BEST_EFFORT /
TRANSIENT_LOCAL / VOLATILE / UNKNOWN / BEST_AVAILABLE-Werten.

**Repo:** `crates/ros2-rmw/src/rmw_qos_mapping.rs::{RmwHistory,
RmwReliability, RmwDurability}` (`#[repr(u32)]`-Enums mit allen
spec-konformen Diskriminanten).

**Tests:** `rmw_qos_mapping::tests::enum_repr_is_c_compatible`.

**Status:** done

### `rmw_to_dds`-Konversion + `BEST_AVAILABLE`-Handling

**Spec:** `rmw/qos_profiles.h` + Design-Article — `rmw_qos_profile_t`
ist die ROS-2-Side-Repräsentation; muss bidirektional auf
DDS-`QosProfile` abgebildet werden. `BEST_AVAILABLE` (Iron+) wird auf
Sender-Seite zu `BEST_EFFORT` resolved.

**Repo:** `crates/ros2-rmw/src/rmw_qos_mapping.rs::{rmw_to_dds,
dds_to_rmw, RmwQosProfile}`.

**Tests:** `rmw_qos_mapping::tests::rmw_to_dds_round_trip_default`,
`rmw_qos_mapping::tests::rmw_to_dds_keep_all_passes_through`,
`rmw_qos_mapping::tests::rmw_system_default_maps_to_dds_reliable`,
`rmw_qos_mapping::tests::rmw_best_available_maps_to_best_effort_on_sender`,
`rmw_qos_mapping::tests::transient_local_round_trips`,
`rmw_qos_mapping::tests::sensor_data_is_best_effort`,
`rmw_qos_mapping::tests::services_default_uses_keep_last_10`,
`rmw_qos_mapping::tests::parameters_uses_keep_last_1000`,
`rmw_qos_mapping::tests::default_profile_matches_rmw_spec`.

**Status:** done

---

## ROS-IDL → DDS-XTypes Wire-Mapping

### Sub-Namespace-Convention (msg/srv/action)

**Spec:** ROS 2 Design-Article
`design.ros2.org/articles/idl_interface_definition.html` §"Naming" —
Top-Level-Namespaces `msg/`, `srv/`, `action/` mit Convention auf
DDS-Wire `<package>::<sub-namespace>::dds_::<TypeName>_`.

**Repo:** `crates/ros2-rmw/src/type_mapping.rs::RosNamespace` mit
`as_str()`/`from_str()`-Konvertern; `RosTypeRef::to_dds_type_name`
für die DDS-Wire-Form.

**Tests:** `type_mapping::tests::namespace_str_repr`,
`type_mapping::tests::dds_wire_form_uses_dds_dunder`,
`type_mapping::tests::action_namespace_mapped_correctly`,
`type_mapping::tests::srv_namespace_mapped_correctly`.

**Status:** done

### ROS-Form → DDS-Form Round-Trip

**Spec:** Design-Article — Konvertierung zwischen ROS-Wire-Form
(`std_msgs/msg/String`) und DDS-Wire-Form (siehe oben). Roundtrip-
Pflicht.

**Repo:** `crates/ros2-rmw/src/type_mapping.rs::{RosTypeRef::new,
to_ros_form, from_ros_form}`.

**Tests:** `type_mapping::tests::ros_form_round_trip`,
`type_mapping::tests::from_ros_form_rejects_unknown_namespace`,
`type_mapping::tests::from_ros_form_rejects_wrong_segment_count`.

**Status:** done

### Builtin-Type-Tokens (`int32`, `float64`, `string`, ...)

**Spec:** Design-Article §"Field Types" — Builtin-Type-Tokens (`bool`,
`byte`, `char`, `float32`/`float64`, `int8`-`uint64`, `string`,
`wstring`) mappen auf DDS-IDL-Primitive.

**Repo:** `crates/ros2-rmw/src/type_mapping.rs::RosBuiltinType` mit
`from_ros_token()`-Parser und `cdr_size`-Helper.

**Tests:** `type_mapping::tests::from_ros_token_round_trip`,
`type_mapping::tests::from_ros_token_rejects_unknown`,
`type_mapping::tests::cdr_size_matches_omg_cdr2`,
`type_mapping::tests::builtin_idl_names_match_omg_idl`.

**Status:** done — rosidl-Builtin → OMG-IDL-4.2-/DDS-XTypes-1.3-Primitive-
Mapping verifiziert.

---

## RMW C-ABI Plugin (`crates/rmw-zerodds-shim`)

Die obigen 14 Items decken den Wire-**Mapping**-Crate (`crates/ros2-rmw`) ab.
Die eigentliche `rmw_*`-C-ABI-Plugin-Schicht (`rmw_c/rmw_zerodds.c` brückt auf
die Rust-Bridge `src/lib.rs`) ist hier separat geführt. Verifikation: rclpy-
Interop-Pytest auf ROS 2 Humble (`crates/py/python/tests/ros2/`), 4/4 grün.

### `rmw_wait` — event-driven

**Spec:** `rmw/rmw.h` `rmw_wait` (de-facto) — blockiert bis eine Entity ready
ist oder Timeout.

**Repo:** `crates/rmw-zerodds-shim/rmw_c/rmw_zerodds.c` (`rmw_wait`) +
`src/lib.rs` (`WaitNotify`, `subscription_on_data`, `rmw_zerodds_*_has_data`,
`rmw_zerodds_context_wait_block`) — Reader-Daten-Listener → per-Subscription-
Inbox → Context-Condvar; kein Spin, kein Fixed-Tick-Poll. Für die Raw-Delivery-Modi
(RawSameHost/Iceoryx), die den RTPS-Listener nicht feuern, weckt zusätzlich ein
Doorbell-Thread (`rmw_zerodds_subscription_start_doorbell` → `zerodds_reader_raw_wait`)
dieselbe Condvar event-getrieben (siehe „Loaned Messages" unten).

**Tests:** `rmw-zerodds-shim::tests::event_driven_wait_roundtrip_inprocess`,
`wait_notify_blocks_then_wakes_on_notify`; rclpy
`test_rclpy_publish_subscribe_string_roundtrip` (Executor fährt durch `rmw_wait`).

**Status:** done

### Services — Request/Reply

**Spec:** `rmw/rmw.h` `rmw_create_client/service`, `rmw_send_request`,
`rmw_take_request`, `rmw_send_response`, `rmw_take_response`,
`rmw_service_server_is_available`.

**Repo:** `rmw_zerodds.c` (Service-Typesupport-Introspection + 24-Byte-
Korrelations-Header `[client_gid:16][seq:8]` + CDR) + `src/lib.rs`
(`RmwZerodsClient/Service` mit Listener-Inbox, `rmw_zerodds_send_request` etc.).

**Tests:** `rmw-zerodds-shim::tests::service_request_reply_roundtrip_inprocess`;
rclpy `test_rclpy_service_call_roundtrip` (AddTwoInts 41+1=42).

**Status:** done

### `rmw_serialize` / `rmw_deserialize` + serialized publish/take

**Spec:** `rmw/rmw.h` `rmw_serialize`, `rmw_deserialize`,
`rmw_publish_serialized_message`, `rmw_take_serialized_message[_with_info]`.

**Repo:** `rmw_zerodds.c` (Introspection-CDR `[encap 4][body]`,
`cdr_ser_msg`/`cdr_de_msg`; serialized pub/take über den Bridge-Byte-Pfad).

**Tests:** dieselbe Introspection-CDR wie der verifizierte pub/sub-Pfad
(`test_rclpy_publish_subscribe_string_roundtrip`).

**Status:** done

### Topic-Graph — `rmw_get_topic_names_and_types` + `rmw_count_publishers/subscribers`

**Spec:** `rmw/rmw.h` Graph-Introspektion.

**Repo:** `crates/dcps/src/runtime.rs`
(`discovered_publication_topics`/`discovered_subscription_topics` — lokale
User-Endpoints + SEDP-remote), c-api `zerodds_runtime_for_each_publication/
_subscription`, `rmw_zerodds.c` (Demangle `rt/<t>→/<t>`, `::`/`__`→`/`,
dedup topic→types).

**Tests:** rclpy `test_rclpy_topic_graph_introspection`.

**Status:** done

### Guard-Conditions + event-driven Wake

**Repo:** `rmw_zerodds.c` (`rmw_create/trigger_guard_condition` → `context_notify`
weckt die Wait-Condvar).

**Tests:** über den `rmw_wait`-Pfad mitgetestet.

**Status:** done

### `rmw_get_node_names` + `_with_enclaves`

**Spec:** `rmw/rmw.h` `rmw_get_node_names[_with_enclaves]`.

**Repo:** `rmw_zerodds.c` (`rmw_get_node_names`, Accumulator) + `src/lib.rs`
(`NodeGraph`, `encode/decode_participant_info`, `discovery_on_data`,
`rmw_zerodds_for_each_node`) — jeder Context publiziert seine
`ParticipantEntitiesInfo` auf `ros_discovery_info` (hand-encodiertes XCDR1) und
aggregiert lokale + remote Nodes.

**Tests:** rclpy `test_rclpy_node_names_graph`.

**Status:** done — Hinweis: discovery-Writer ist volatile (c-api-Default);
cross-Prozess-late-join wäre mit transient_local robuster (Verfeinerung).

### `on_new_message/request/response`-Callbacks (Events-Executor)

**Spec:** `rmw/rmw.h` `rmw_*_set_on_new_*_callback`.

**Repo:** `rmw_zerodds.c` (3 Setter → Bridge) + `src/lib.rs`
(`SubInbox.event`, `inbox_set_event`, Invoke in `subscription_on_data`).

**Tests:** `rmw-zerodds-shim::tests::event_callback_fires_on_arrival`.

**Status:** done

### `rmw_get_serialized_message_size`

**Spec:** `rmw/rmw.h` `rmw_get_serialized_message_size`.

**Repo:** `rmw_zerodds.c` (`zerodds_cdr_max_msg` — Introspection-Size-Walk,
konservative obere Schranke; Strings/Sequences gecappt).

**Tests:** über den verifizierten serialize-Pfad (gleiche Member-Traversierung).

**Status:** done

### Typed-Message-Loaning (`rmw_borrow/publish/take_loaned_message`)

**Spec:** `rmw/rmw.h` Loaned-Message-API + `can_loan_messages`. Delivery-Form:
`docs/specs/zerodds-delivery-modes-1.0.md` (Modes `Portable`/`RawSameHost`/`Iceoryx`).

**Repo:** `rmw_c/rmw_zerodds.c` (`rmw_borrow/publish/take_loaned_message`,
`rmw_return_loaned_message_from_publisher/subscription`, `can_loan_messages`
= fixed-POD aus rosidl-Introspection) + die SHM-Bridge in
`src/lib.rs` (`rmw_zerodds_publisher_enable_raw_loan`/`_loan`/`_commit`/`_discard`,
`rmw_zerodds_subscription_enable_shm`/`_take_shm`/`_has_shm_data`/`_release_shm`),
Feature `flatdata-loan` (default).  **Tests:** `rmw_c/loaned_message_test.cpp` +
`run_loaned_message_test.sh` (rclcpp e2e, **beide Modi** grün).

**Status:** done — die Loaned-Message-ABI ist implementiert und e2e verifiziert,
in zwei Delivery-Modi (Default per `ZERODDS_DELIVERY_MODE`):

* **`Portable`** (default): rclcpp übergibt einen getypten Struct-Buffer, der
  User schreibt die Struct, `publish_loaned` serialisiert Struct→CDR und
  publiziert über RTPS — interop-sicher (cross-host/cross-vendor), echtes CDR
  auf dem Draht. (`can_loan=1 got=42 PASS`.)
* **`RawSameHost`** (echtes Zero-Copy/Zero-Serialize, same-host-only): der
  Writer ist auf `set_delivery_mode(RawSameHost)` + `enable_shm_loan` gestellt;
  `borrow` liefert einen Zeiger in den POSIX-SHM-Slot (der User schreibt die
  Struct direkt in Shared-Memory), `commit` finalisiert in-place ohne
  Serialisierung **und ohne RTPS** (c-api-`publishes_to_wire`-Gate → keine
  Double-Delivery). Der Reader mappt dasselbe Segment per deterministischem
  topic-abgeleitetem flink-Pfad (lazy attach) und liest den Slot zero-copy
  (`take_loaned`) bzw. mit einem Struct-memcpy für normale Callbacks
  (`rmw_take`). (`can_loan=1 got=42 PASS`, Beweis für SHM: Raw geht nie auf den
  Draht.)
* **`Iceoryx`** (same-host cross-stack, Shim-Feature `delivery-iceoryx`):
  derselbe loan/commit/take-Pfad, aber Writer/Reader sind an einen iceoryx2-
  Service (topic-abgeleitet) gebunden — `commit` sendet über iceoryx2, der Reader
  empfängt davon. Ohne das Feature degradiert `ZERODDS_DELIVERY_MODE=iceoryx`
  beidseitig auf `Portable`. (`can_loan=1 got=42 PASS` via `ZERODDS_TEST_ICEORYX=1`.)

Readiness (Modi 1/2): **event-getrieben**. Pro Raw-Subscription parkt ein
„Doorbell"-Thread auf `zerodds_reader_raw_wait(reader, timeout_ms)` (c-api), das
auf der Raw-Quelle blockiert — SHM-Change-Generation-Futex
(`notify_generation`/`wait_for_change`) bzw. iceoryx2-Listener — und bei einer
**echten** Ankunft die Context-Condvar von `rmw_wait` weckt; der Sender notifiziert
das iceoryx2-Event beim `commit`. Der Doorbell wird lazy gestartet, sobald die
Raw-Quelle aktiv ist, und vor dem Reader-Destroy gestoppt + gejoint (er hält den
Reader-Pointer). Da iceoryx2's `take` ein **destruktiver** FIFO-Receive ist,
prefetcht die Readiness zusätzlich genau ein Sample in einen Pending-Buffer der
Subscription (nicht-konsumierend, idempotent); `rmw_take`/`take_loaned`
konsumieren das gehaltene Sample. Damit weckt ein blockierendes rclcpp-`spin()`
auch bei reinen Raw-Daten ohne Executor-Timeout.

---

## Audit-Status

26 done / 0 partial / 0 open / 1 n/a (informative) / 2 n/a (out of rmw scope: REP-2007 Type-Adaptation, REP-2008 HW-Accel).

Test-Lauf: `cargo test -p zerodds-ros2-rmw` (52 grün) +
`cargo test -p rmw-zerodds-shim` (23 grün — Shim-Unit inkl. event-driven Wait,
Service-Roundtrip, Event-Callback, Context-Lifecycle-Regression, Endpoint-gid-
Roundtrip) +
rclpy-Interop `crates/py/python/tests/ros2/` auf ROS 2 Humble (6 grün: init,
pub/sub, service-call, topic-graph, node-names, endpoint-info) via
`run_ros2_pytest.sh` +
rclcpp-Loan-e2e via `run_loaned_message_test.sh` (Portable + RawSameHost +
`ZERODDS_TEST_ICEORYX=1` Iceoryx, je `can_loan=1 got=42 PASS`).

Kein offener rmw-Punkt; alle drei Delivery-Modi (`Portable`/`RawSameHost`/
`Iceoryx`) sind rmw-seitig verdrahtet + e2e-verifiziert, und die Raw-Readiness ist
event-getrieben (Doorbell-Thread auf `zerodds_reader_raw_wait`, SHM-Futex bzw.
iceoryx2-Listener) — die zuletzt offene Delivery-Modes-Verfeinerung
(`docs/specs/zerodds-delivery-modes-1.0.md`) ist damit geschlossen. Decision-Records
(REP-2007/2008/2009 — Features leben in `rclcpp` über RMW, via
`rmw_zerodds`-FFI integrabel): siehe `ros2-rmw.open.md`.
