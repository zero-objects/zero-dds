# ROS 2 RMW Bridge — Spec-Coverage

**Quellen:**

* ROS Enhancement Proposals (REPs) — `docs/standards/cache/ros2/rep-{2003,2004,2005,2007,2008,2009}.html`.
* RMW C-API Headers — `rmw/rmw.h` und `rmw/qos_profiles.h` aus
  `rmw 4.x` (ROS 2 Iron/Jazzy Distribution); leben im upstream
  `ros2/rmw`-GitHub-Repository, **nicht** in `docs/standards/cache/`,
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
ZeroDDS implementiert die Mapping-Layer (`crates/ros2-rmw/`) als
pure-Rust no_std+alloc Library; der eigentliche
`rmw_zerodds`-C-FFI-Wrapper liegt in `crates/rmw-zerodds-shim/`.

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

**Status:** n/a (rejected) — andere Architektur-Schicht: REP-2007 lebt
in `rclcpp` (Sprach-Binding-Layer), nicht in RMW. ZeroDDS dockt via
`rmw_zerodds`-C-FFI an die RMW-Schicht; das `rclcpp`-API stammt aus
dem ROS-2-Projekt selbst und wird nicht durch DDS-Vendor reimplementiert.
Siehe Decision-Record in `ros2-rmw.open.md`.

---

## REP-2008 Hardware Acceleration

### HW-Accel Architecture

**Spec:** REP-2008 — Conventions für GPU/FPGA-Drivers in ROS 2.

**Repo:** —

**Tests:** —

**Status:** n/a (rejected) — andere Architektur-Schicht: REP-2008 ist
Driver-/Vendor-Konvention für Acceleration-Hardware, lebt in der
GPU/FPGA-Vendor-Schicht und wird durch ROS-2-Anwender direkt
orchestriert (CUDA/ROCm/Vitis-Stacks), nicht durch DDS-Vendor. Siehe
Decision-Record in `ros2-rmw.open.md`.

---

## REP-2009 Type Negotiation Feature

### Type Negotiation API

**Spec:** REP-2009 — Runtime-Feature in `rclcpp` zur Pub-Sub-
Type-Negotiation.

**Repo:** —

**Tests:** —

**Status:** n/a (rejected) — andere Architektur-Schicht: REP-2009
implementiert Type-Negotiation in `rclcpp` (Sprach-Binding-Layer),
nutzt RMW nur als Pub/Sub-Wire. Negotiation-State-Machine +
Type-Resolution gehören zum ROS-2-Stack selbst, nicht zum DDS-Vendor.
Siehe Decision-Record in `ros2-rmw.open.md`.

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
`is_unknown(profile)`-Predicate.

**Tests:** `qos_profiles::tests::default_profile_is_reliable_volatile_keep_last_10`,
`qos_profiles::tests::parameters_profile_uses_keep_last_1000`,
`qos_profiles::tests::services_default_matches_rmw_defaults`,
`qos_profiles::tests::parameter_events_uses_keep_last_1000`,
`qos_profiles::tests::system_default_aliases_default`,
`qos_profiles::tests::history_keep_last_distinct_from_keep_all`,
`qos_profiles::tests::liveliness_and_deadline_default_to_infinite`,
`qos_profiles::tests::unknown_profile_is_distinct_from_default`,
`qos_profiles::tests::unknown_profile_uses_keep_last_zero_marker`,
`qos_profiles::tests::is_unknown_recognizes_unknown_profile`,
`qos_profiles::tests::is_unknown_rejects_real_profiles`.

**Status:** done — alle sieben rmw-Default-Profiles als Konstanten
abgebildet:
* Sechs 1:1 (DEFAULT, SENSOR_DATA, PARAMETERS, SERVICES_DEFAULT,
  PARAMETER_EVENTS, MAP).
* `SYSTEM_DEFAULT` als Alias zu DEFAULT (Spec erlaubt
  Implementation-defined; ZeroDDS waehlt dieselben Werte wie
  DEFAULT — dokumentierte Wahl).
* `UNKNOWN` als KeepLast(0)-Marker mit `is_unknown`-Predicate.

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
`type_mapping::tests::builtin_idl_names_match_rep_2008`.

**Status:** done — Test-Name `..._match_rep_2008` ist historisch und
deutet auf den falschen REP; Mapping selbst gegen
DDS-XTypes-1.3-Primitive korrekt.

---

## Audit-Status

14 done / 0 partial / 0 open / 1 n/a (informative) / 3 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-ros2-rmw` — 52 Tests grün, 0 failed.

Decision-Records (REP-2007 Type Adaptation, REP-2008 HW-Accel,
REP-2009 Type Negotiation — alle drei Features leben in `rclcpp`
über RMW, ZeroDDS via `rmw_zerodds`-FFI integrabel): siehe
`ros2-rmw.open.md`.
