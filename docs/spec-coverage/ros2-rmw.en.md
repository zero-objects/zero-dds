# ROS 2 RMW Bridge — Spec Coverage

**Sources:**

* ROS Enhancement Proposals (REPs) — `internal/standards/cache/ros2/rep-{2003,2004,2005,2007,2008,2009}.html`.
* RMW C-API headers — `rmw/rmw.h` and `rmw/qos_profiles.h` from
  `rmw 4.x` (ROS 2 Iron/Jazzy distribution); they live in the upstream
  `ros2/rmw` GitHub repository, **not** in `internal/standards/cache/`, as
  they are a standalone distribution. A de-facto specification without
  its own REP, normative via the header definitions.
* ROS 2 IDL subset — the design article `design.ros2.org/articles/idl_interface_definition.html`
  (not cached; live source) + the wire-naming convention from
  `rosidl_typesupport_fastrtps_cpp` (de-facto, in the `ros2/rosidl`
  GitHub repository).
* Topic/service naming convention — the design article
  `design.ros2.org/articles/topic_and_service_names.html` (not cached;
  live source). Implemented in `rmw_dds_common`.

**Context:** ROS 2 robot-middleware wrappers build on DDS. The RMW API +
the RMW-QoS-profile mapping + the topic-mangling convention + the
ROS-IDL→DDS-XTypes wire convention are the central wire mappings. Spread
across:

- `crates/ros2-rmw/` — mapping layer as a pure-Rust no_std+alloc library (RMW API, QoS-profile mapping, topic mangling, ROS-IDL→DDS-XTypes)
- `crates/rmw-zerodds-shim/` — the `rmw_zerodds` C-FFI wrapper

**Crate mapping:**

| Spec area | Crate file |
|---|---|
| REP-2003 Sensor/Map QoS | `crates/ros2-rmw/src/qos_profiles.rs` (profile constants) |
| REP-2004 quality levels | `crates/ros2-rmw/src/quality.rs` |
| topic-name mangling convention | `crates/ros2-rmw/src/topic_mangling.rs` |
| standard RMW QoS profiles | `crates/ros2-rmw/src/qos_profiles.rs::profiles::*` |
| RMW C-API (`rmw/rmw.h`) | `crates/ros2-rmw/src/ffi_api.rs` |
| RMW-QoS mapping (`rmw/qos_profiles.h`) | `crates/ros2-rmw/src/rmw_qos_mapping.rs` |
| ROS-IDL → DDS-XTypes wire mapping | `crates/ros2-rmw/src/type_mapping.rs` |

Implementation: `crates/ros2-rmw/` (6 modules, 52 tests green via
`cargo test -p zerodds-ros2-rmw`).

---

## REP-2003 Sensor Data and Map QoS Settings

### Map QoS

**Spec:** REP-2003 §"Map Quality of Service" — "Map providers [...] are
expected to provide all maps over a reliable transient-local topic.
[...] The depth of the transient-local storage depth is left to the
designer, however a single map depth is a reasonable choice".

**Repo:** `crates/ros2-rmw/src/qos_profiles.rs::profiles::MAP` — Reliable
+ TransientLocal + KeepLast(1).

**Tests:** `qos_profiles::tests::map_profile_matches_rep_2003_specification`.

**Status:** done

### Sensor Data QoS (consumer-side)

**Spec:** REP-2003 §"Sensor Driver Quality of Service" — verbatim:
"Sensor data provided by a sensor driver from a camera, inertial
measurement unit, laser scanner, GPS, depth, range finder, or other
sensors are expected to be provided over a `SystemDefaultsQoS`
quality of service as provided by the implemented ROS 2 version API.
Consumers of sensor data are to use `SensorDataQoS` quality of service
as provided by the implemented ROS 2 version API."

Important: REP-2003 requires `SystemDefaultsQoS` for the **driver-side**
(= Reliable+Volatile+KeepLast(10), see the `DEFAULT` item under "Standard
RMW QoS Profiles" further below). The consumer-side uses `SensorDataQoS`
(= BestEffort+Volatile+KeepLast(5)). This item describes the
consumer-side; the driver-side is covered via `profiles::DEFAULT`.

**Repo:** `crates/ros2-rmw/src/qos_profiles.rs::profiles::SENSOR_DATA` —
BestEffort + Volatile + KeepLast(5) (corresponds to
`rmw_qos_profile_sensor_data` from `rmw/qos_profiles.h`).

**Tests:** `qos_profiles::tests::sensor_data_profile_matches_rep_2003_specification`
checks the consumer-side constant (BestEffort+Volatile+KeepLast(5)).

**Status:** done — consumer-side covered; the driver-side is equally
covered via the `DEFAULT` profile (see below).

---

## REP-2004 Package Quality Categories

### Quality levels Q1-Q5

**Spec:** REP-2004 — five levels with the following definitions (quoted
from REP-2004 §"Quality Level Categories"):

* **Quality Level 1** — "highest quality level; packages that are needed
  for production systems".
* **Quality Level 2** — "high quality packages that are either: needed for
  production systems or commonly used".
* **Quality Level 3** — "tooling quality packages".
* **Quality Level 4** — "demos, tutorials, and experiments".
* **Quality Level 5** — "default quality level" (packages without explicit
  quality claims).

Numeric representation 1..5; Q1 is the highest, Q5 is the default for
packages without an explicitly declared quality level.

**Repo:** `crates/ros2-rmw/src/quality.rs::QualityLevel` with
`numeric()`/`from_numeric()` converters.

**Tests:** `quality::tests::quality_level_numeric_round_trip`,
`quality::tests::quality_level_ordering_q1_is_highest`,
`quality::tests::quality_level_from_numeric_rejects_out_of_range`.

**Status:** done — the classification model is exposed; the actual quality
audit is a caller task (e.g. a `package.xml` tag).

---

## REP-2005 ROS 2 Common Packages

### Common-package list

**Spec:** REP-2005 — informational; a list of the ROS-2 common packages.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — REP-2005 marks itself as informational; the
common-package list has no normative requirement on the RMW bridge.

---

## REP-2007 Type Adaptation Feature

### Type adaptation API

**Spec:** REP-2007 — a compile-time feature in `rclcpp` (C++) for
converting user types to ROS messages on the fly.

**Repo:** —

**Tests:** —

**Status:** n/a (no rmw surface) — REP-2007 Type Adaptation is a compile-time
`rclcpp` template: the conversion happens **above** rmw (the user type is already
a ROS message before it reaches the wire), so there is nothing to implement at the
rmw layer. The rmw API that REP-2007 itself specifies (node/identifier/QoS
mapping) is fully implemented (all done items above). Not a vendor "reject" — a
layer fact.

---

## REP-2008 Hardware Acceleration

### HW-accel architecture

**Spec:** REP-2008 — conventions for GPU/FPGA drivers in ROS 2.

**Repo:** —

**Tests:** —

**Status:** n/a (out of rmw scope) — REP-2008 is a driver/vendor convention for
acceleration hardware (GPU/FPGA), lives in the hardware-vendor layer and is
orchestrated directly by ROS-2 users (CUDA/ROCm/Vitis), not by the DDS vendor.
Unlike REP-2009 there is no rmw surface to implement here.

---

### Endpoint info by topic — `rmw_get_publishers/subscriptions_info_by_topic`

**Spec:** `rmw/get_topic_endpoint_info.h` — per topic, the endpoint list with
node name/namespace, type, endpoint type, 16-byte GUID and QoS (`ros2 topic
info -v`). Part of the rmw-side REP-2009 responsibility.

**Repo:** `rmw_c/rmw_zerodds.c` (`rmw_get_publishers_info_by_topic`/
`_subscriptions_info_by_topic` → `zerodds_get_endpoint_info_by_topic`): enumerates
per-endpoint info via `rmw_zerodds_node_for_each_publication/subscription_endpoint`,
filters by the demangled topic, resolves each endpoint's node identity via
`rmw_zerodds_node_resolve_endpoint` (endpoint GUID → node), and fills
`rmw_topic_endpoint_info_array_t`. Data path: `crates/dcps/src/runtime.rs`
(`discovered_publication/subscription_endpoints`, local + SEDP-remote) → c-api
(`zerodds_runtime_for_each_*_endpoint` + `ZeroDdsEndpointInfo`) → shim. The
endpoint→node link relies on the `ros_discovery_info` participant gid being the
**real** DDS participant GUID, with `ParticipantEntitiesInfo` carrying per-node
reader/writer gid sequences (the endpoint GUID prefix matches the participant).
QoS is best-effort from discovery (history/depth aren't on the wire → `UNKNOWN`).

**Tests:** `crates/py/python/tests/ros2/test_rmw_zerodds_interop.py::test_rclpy_endpoint_info_by_topic`
(rclpy `get_publishers_info_by_topic`: type + QoS + the node name/namespace
resolved from the endpoint GUID) on ROS 2 Humble; shim roundtrip units
`participant_info_roundtrips_endpoint_gids` / `_without_endpoints`.

**Status:** done

## REP-2009 Type Negotiation Feature

### Type negotiation — rmw-side part (type hash + endpoint info)

**Spec:** REP-2009 — runtime pub/sub type negotiation. The rmw-side
responsibilities are the RIHS type hash and the endpoint type/QoS exposure used
to match negotiating endpoints.

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs::rmw_zerodds_compute_type_hash`
(RIHS SHA-256) + the endpoint-info path (`rmw_get_publishers/subscriptions_info_by_topic`,
see "Endpoint info by topic" above).

**Tests:** `type_hash_sha256_is_deterministic` + 2 more (3) +
`test_rclpy_endpoint_info_by_topic` (Humble).

**Status:** done (rmw-side). The negotiation **state machine** itself is a
runtime feature in `rclcpp` (the language-binding layer), not an rmw API —
available over any RMW; there is no further rmw surface to implement.

---

## Topic-name mangling convention (de-facto, `rmw_dds_common`)

### Prefix convention rt/rq/rr/rs

**Spec:** the design article `design.ros2.org/articles/topic_and_service_names.html`
§"DDS Topic Names" + the de-facto convention from the `ros2/rmw_dds_common`
implementation:

* `rt/<name>` — topic pub-sub (ROS topic).
* `rq/<name>Request` — service request.
* `rr/<name>Reply` — service reply.
* `rs/<name>` — service discovery (legacy, before the REP-2009 structures).

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

**Status:** done — the suffixes (`Request`/`Reply`) remain a caller task
(typically added automatically by the service codegen).

---

## Standard RMW QoS profiles

### `rmw_qos_profile_*` constants

**Spec:** `rmw/qos_profiles.h` (rmw 4.x) — seven default profiles:
`rmw_qos_profile_default` (Reliable+Volatile+KeepLast(10)),
`rmw_qos_profile_sensor_data` (BestEffort+Volatile+KeepLast(5)),
`rmw_qos_profile_parameters` (Reliable+Volatile+KeepLast(1000)),
`rmw_qos_profile_services_default` (Reliable+Volatile+KeepLast(10)),
`rmw_qos_profile_parameter_events` (Reliable+Volatile+KeepLast(1000)),
`rmw_qos_profile_system_default` (all fields as `*_SYSTEM_DEFAULT`
sentinels; implementation-defined), `rmw_qos_profile_unknown` (all fields
as `*_UNKNOWN` sentinels).

**Repo:** `crates/ros2-rmw/src/qos_profiles.rs::profiles::*` (`DEFAULT`,
`SENSOR_DATA`, `PARAMETERS`, `SERVICES_DEFAULT`, `PARAMETER_EVENTS`,
`SYSTEM_DEFAULT`, `UNKNOWN`, `MAP`) + an `is_unknown(profile)` predicate.

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

**Status:** done — all seven rmw default profiles mapped as constants:
* Six 1:1 (DEFAULT, SENSOR_DATA, PARAMETERS, SERVICES_DEFAULT,
  PARAMETER_EVENTS, MAP).
* `SYSTEM_DEFAULT` as an alias of DEFAULT (the spec allows
  implementation-defined; ZeroDDS chooses the same values as DEFAULT — a
  documented choice).
* `UNKNOWN` as a KeepLast(0) marker with the `is_unknown` predicate.

---

## RMW C-API (`rmw/rmw.h`)

### `rmw_ret_t` return codes

**Spec:** `rmw/types.h` (rmw 4.x) — `rmw_ret_t` as an `int32` with the
values OK=0, ERROR=1, TIMEOUT=2, UNSUPPORTED=3, BAD_ALLOC=10,
INVALID_ARGUMENT=11, INCORRECT_RMW_IMPLEMENTATION=12.

**Repo:** `crates/ros2-rmw/src/ffi_api.rs::RmwRet` (`#[repr(i32)]` enum
with all seven values + a `map_to_rmw_ret` converter).

**Tests:** `ffi_api::tests::error_codes_match_rmw_h`,
`ffi_api::tests::ok_is_zero`, `ffi_api::tests::map_to_rmw_ret_ok`,
`ffi_api::tests::map_to_rmw_ret_err`.

**Status:** done

### Implementation identifier (`rmw_get_implementation_identifier`)

**Spec:** `rmw/rmw.h` — `rmw_get_implementation_identifier()` returns a
vendor string (convention `rmw_<vendor>_cpp`, e.g.
`"rmw_fastrtps_cpp"`/`"rmw_cyclonedds_cpp"`/`"rmw_connext_cpp"`).
The `RMW_CHECK_*_FOR_NULL_*` macros (in
`rmw/check_type_identifiers_match.h`) check caller-passed strings against
this identifier (`RMW_RET_INCORRECT_RMW_IMPLEMENTATION` on a mismatch).

**Repo:** `crates/ros2-rmw/src/ffi_api.rs::check_rmw_identifier` plus the
constant `RMW_IMPLEMENTATION_IDENTIFIER: &str = "rmw_zerodds_cpp"` (follows
the vendor-naming convention).

**Tests:** `ffi_api::tests::implementation_identifier_matches_convention`,
`ffi_api::tests::check_rmw_identifier_accepts_correct`,
`ffi_api::tests::check_rmw_identifier_rejects_other_vendor`.

**Status:** done

### `rmw_node_t` construction

**Spec:** `rmw/types.h` — `rmw_node_t` struct with an
`implementation_identifier` field + `name`/`namespace`/`context`.

**Repo:** `crates/ros2-rmw/src/ffi_api.rs::RmwNode`.

**Tests:** `ffi_api::tests::rmw_node_construction`.

**Status:** done

---

## RMW-QoS mapping (`rmw/qos_profiles.h`)

### `rmw_qos_*_policy_t` enums (History/Reliability/Durability)

**Spec:** `rmw/qos_profiles.h` — three C enums
(`rmw_qos_history_policy_t`, `rmw_qos_reliability_policy_t`,
`rmw_qos_durability_policy_t`) with
SYSTEM_DEFAULT / KEEP_LAST / KEEP_ALL / RELIABLE / BEST_EFFORT /
TRANSIENT_LOCAL / VOLATILE / UNKNOWN / BEST_AVAILABLE values.

**Repo:** `crates/ros2-rmw/src/rmw_qos_mapping.rs::{RmwHistory,
RmwReliability, RmwDurability}` (`#[repr(u32)]` enums with all
spec-conformant discriminants).

**Tests:** `rmw_qos_mapping::tests::enum_repr_is_c_compatible`.

**Status:** done

### `rmw_to_dds` conversion + `BEST_AVAILABLE` handling

**Spec:** `rmw/qos_profiles.h` + design article — `rmw_qos_profile_t` is
the ROS-2-side representation; it must be mapped bidirectionally onto the
DDS `QosProfile`. `BEST_AVAILABLE` (Iron+) is resolved to `BEST_EFFORT` on
the sender side.

**Repo:** `crates/ros2-rmw/src/rmw_qos_mapping.rs::{rmw_to_dds, dds_to_rmw,
RmwQosProfile}`.

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

## ROS-IDL → DDS-XTypes wire mapping

### Sub-namespace convention (msg/srv/action)

**Spec:** the ROS 2 design article
`design.ros2.org/articles/idl_interface_definition.html` §"Naming" —
top-level namespaces `msg/`, `srv/`, `action/` with the convention onto the
DDS wire `<package>::<sub-namespace>::dds_::<TypeName>_`.

**Repo:** `crates/ros2-rmw/src/type_mapping.rs::RosNamespace` with
`as_str()`/`from_str()` converters; `RosTypeRef::to_dds_type_name` for the
DDS wire form.

**Tests:** `type_mapping::tests::namespace_str_repr`,
`type_mapping::tests::dds_wire_form_uses_dds_dunder`,
`type_mapping::tests::action_namespace_mapped_correctly`,
`type_mapping::tests::srv_namespace_mapped_correctly`.

**Status:** done

### ROS-form → DDS-form round-trip

**Spec:** design article — conversion between the ROS wire form
(`std_msgs/msg/String`) and the DDS wire form (see above). Round-trip
mandatory.

**Repo:** `crates/ros2-rmw/src/type_mapping.rs::{RosTypeRef::new,
to_ros_form, from_ros_form}`.

**Tests:** `type_mapping::tests::ros_form_round_trip`,
`type_mapping::tests::from_ros_form_rejects_unknown_namespace`,
`type_mapping::tests::from_ros_form_rejects_wrong_segment_count`.

**Status:** done

### Builtin type tokens (`int32`, `float64`, `string`, ...)

**Spec:** design article §"Field Types" — builtin type tokens (`bool`,
`byte`, `char`, `float32`/`float64`, `int8`-`uint64`, `string`, `wstring`)
map onto the DDS-IDL primitives.

**Repo:** `crates/ros2-rmw/src/type_mapping.rs::RosBuiltinType` with a
`from_ros_token()` parser and a `cdr_size` helper.

**Tests:** `type_mapping::tests::from_ros_token_round_trip`,
`type_mapping::tests::from_ros_token_rejects_unknown`,
`type_mapping::tests::cdr_size_matches_omg_cdr2`,
`type_mapping::tests::builtin_idl_names_match_omg_idl`.

**Status:** done — rosidl builtin → OMG IDL 4.2 / DDS-XTypes 1.3 primitive
mapping verified.

---

## RMW C-ABI plugin (`crates/rmw-zerodds-shim`)

The 14 items above cover the wire-**mapping** crate (`crates/ros2-rmw`). The actual
`rmw_*` C-ABI plugin layer (`rmw_c/rmw_zerodds.c` bridges onto the Rust bridge
`src/lib.rs`) is tracked separately here. Verification: rclpy interop pytest on
ROS 2 Humble (`crates/py/python/tests/ros2/`), 4/4 green.

### `rmw_wait` — event-driven

**Spec:** `rmw/rmw.h` `rmw_wait` (de facto) — blocks until an entity is ready or
times out.

**Repo:** `crates/rmw-zerodds-shim/rmw_c/rmw_zerodds.c` (`rmw_wait`) +
`src/lib.rs` (`WaitNotify`, `subscription_on_data`, `rmw_zerodds_*_has_data`,
`rmw_zerodds_context_wait_block`) — reader data listener → per-subscription
inbox → context condvar; no spin, no fixed-tick poll. For the raw delivery modes
(RawSameHost/Iceoryx), which do not fire the RTPS listener, a doorbell thread
(`rmw_zerodds_subscription_start_doorbell` → `zerodds_reader_raw_wait`)
additionally wakes the same condvar event-driven (see "Loaned messages" below).

**Tests:** `rmw-zerodds-shim::tests::event_driven_wait_roundtrip_inprocess`,
`wait_notify_blocks_then_wakes_on_notify`; rclpy
`test_rclpy_publish_subscribe_string_roundtrip` (the executor drives `rmw_wait`).

**Status:** done

### Services — request/reply

**Spec:** `rmw/rmw.h` `rmw_create_client/service`, `rmw_send_request`,
`rmw_take_request`, `rmw_send_response`, `rmw_take_response`,
`rmw_service_server_is_available`.

**Repo:** `rmw_zerodds.c` (service typesupport introspection + 24-byte
correlation header `[client_gid:16][seq:8]` + CDR) + `src/lib.rs`
(`RmwZerodsClient/Service` with a listener inbox, `rmw_zerodds_send_request` etc.).

**Tests:** `rmw-zerodds-shim::tests::service_request_reply_roundtrip_inprocess`;
rclpy `test_rclpy_service_call_roundtrip` (AddTwoInts 41+1=42).

**Status:** done

### `rmw_serialize` / `rmw_deserialize` + serialized publish/take

**Spec:** `rmw/rmw.h` `rmw_serialize`, `rmw_deserialize`,
`rmw_publish_serialized_message`, `rmw_take_serialized_message[_with_info]`.

**Repo:** `rmw_zerodds.c` (introspection CDR `[encap 4][body]`,
`cdr_ser_msg`/`cdr_de_msg`; serialized pub/take over the bridge byte path).

**Tests:** the same introspection CDR as the verified pub/sub path
(`test_rclpy_publish_subscribe_string_roundtrip`).

**Status:** done

### Topic graph — `rmw_get_topic_names_and_types` + `rmw_count_publishers/subscribers`

**Spec:** `rmw/rmw.h` graph introspection.

**Repo:** `crates/dcps/src/runtime.rs`
(`discovered_publication_topics`/`discovered_subscription_topics` — local user
endpoints + SEDP-remote), c-api `zerodds_runtime_for_each_publication/
_subscription`, `rmw_zerodds.c` (demangle `rt/<t>→/<t>`, `::`/`__`→`/`,
dedup topic→types).

**Tests:** rclpy `test_rclpy_topic_graph_introspection`.

**Status:** done

### Guard conditions + event-driven wake

**Repo:** `rmw_zerodds.c` (`rmw_create/trigger_guard_condition` → `context_notify`
wakes the wait condvar).

**Tests:** covered via the `rmw_wait` path.

**Status:** done

### `rmw_get_node_names` + `_with_enclaves`

**Spec:** `rmw/rmw.h` `rmw_get_node_names[_with_enclaves]`.

**Repo:** `rmw_zerodds.c` (`rmw_get_node_names`, accumulator) + `src/lib.rs`
(`NodeGraph`, `encode/decode_participant_info`, `discovery_on_data`,
`rmw_zerodds_for_each_node`) — each context publishes its
`ParticipantEntitiesInfo` on `ros_discovery_info` (hand-encoded XCDR1) and
aggregates local + remote nodes.

**Tests:** rclpy `test_rclpy_node_names_graph`.

**Status:** done — note: the discovery writer is volatile (c-api default);
cross-process late-join would be more robust with transient_local (refinement).

### `on_new_message/request/response` callbacks (events executor)

**Spec:** `rmw/rmw.h` `rmw_*_set_on_new_*_callback`.

**Repo:** `rmw_zerodds.c` (3 setters → bridge) + `src/lib.rs`
(`SubInbox.event`, `inbox_set_event`, invoked in `subscription_on_data`).

**Tests:** `rmw-zerodds-shim::tests::event_callback_fires_on_arrival`.

**Status:** done

### `rmw_get_serialized_message_size`

**Spec:** `rmw/rmw.h` `rmw_get_serialized_message_size`.

**Repo:** `rmw_zerodds.c` (`zerodds_cdr_max_msg` — introspection size walk,
conservative upper bound; strings/sequences capped).

**Tests:** via the verified serialize path (same member traversal).

**Status:** done

### Typed message loaning (`rmw_borrow/publish/take_loaned_message`)

**Spec:** `rmw/rmw.h` loaned-message API + `can_loan_messages`. Delivery shape:
`docs/specs/zerodds-delivery-modes-1.0.md` (modes `Portable`/`RawSameHost`/`Iceoryx`).

**Repo:** `rmw_c/rmw_zerodds.c` (`rmw_borrow/publish/take_loaned_message`,
`rmw_return_loaned_message_from_publisher/subscription`, `can_loan_messages`
= fixed-POD from rosidl introspection) + the SHM bridge in
`src/lib.rs` (`rmw_zerodds_publisher_enable_raw_loan`/`_loan`/`_commit`/`_discard`,
`rmw_zerodds_subscription_enable_shm`/`_take_shm`/`_has_shm_data`/`_release_shm`),
feature `flatdata-loan` (default).  **Tests:** `rmw_c/loaned_message_test.cpp` +
`run_loaned_message_test.sh` (rclcpp e2e, **both modes** green).

**Status:** done — the loaned-message ABI is implemented and e2e verified, in two
delivery modes (default via `ZERODDS_DELIVERY_MODE`):

* **`Portable`** (default): rclcpp hands over a typed struct buffer, the user
  writes the struct, `publish_loaned` serializes struct→CDR and publishes over
  RTPS — interop-safe (cross-host/cross-vendor), real CDR on the wire.
  (`can_loan=1 got=42 PASS`.)
* **`RawSameHost`** (real zero-copy/zero-serialize, same-host only): the writer
  is set to `set_delivery_mode(RawSameHost)` + `enable_shm_loan`; `borrow` returns
  a pointer into the POSIX-SHM slot (the user writes the struct directly into
  shared memory), `commit` finalizes in-place without serialization **and without
  RTPS** (c-api `publishes_to_wire` gate → no double-delivery). The reader maps the
  same segment via a deterministic topic-derived flink path (lazy attach) and reads
  the slot zero-copy (`take_loaned`) or with a struct memcpy for normal callbacks
  (`rmw_take`). (`can_loan=1 got=42 PASS`, proof of SHM: raw never goes on the
  wire.)
* **`Iceoryx`** (same-host cross-stack, shim feature `delivery-iceoryx`): the same
  loan/commit/take path, but writer/reader are bound to an iceoryx2 service
  (topic-derived) — `commit` sends over iceoryx2, the reader receives from it.
  Without the feature, `ZERODDS_DELIVERY_MODE=iceoryx` degrades on both sides to
  `Portable`. (`can_loan=1 got=42 PASS` via `ZERODDS_TEST_ICEORYX=1`.)

Readiness (modes 1/2): **event-driven**. Per raw subscription a "doorbell" thread
parks on `zerodds_reader_raw_wait(reader, timeout_ms)` (c-api), which blocks on the
raw source — SHM change-generation futex (`notify_generation`/`wait_for_change`) or
iceoryx2 listener — and wakes `rmw_wait`'s context condvar on a **real** arrival;
the sender notifies the iceoryx2 event on `commit`. The doorbell is started lazily
once the raw source is active and stopped + joined before the reader is destroyed
(it holds the reader pointer). Since iceoryx2's `take` is a **destructive** FIFO
receive, readiness additionally prefetches exactly one sample into a
non-consuming, idempotent pending buffer of the subscription; `rmw_take`/
`take_loaned` consume the held sample. So a blocking rclcpp `spin()` also wakes on
raw-only data without the executor timeout.

---

## Audit status

26 done / 0 partial / 0 open / 1 n/a (informative) / 2 n/a (out of rmw scope: REP-2007 type adaptation, REP-2008 HW accel).

Test run: `cargo test -p zerodds-ros2-rmw` (52 green) +
`cargo test -p rmw-zerodds-shim` (23 green — shim unit incl. event-driven wait,
service roundtrip, event callback, context-lifecycle regression, endpoint-gid
roundtrip) +
rclpy interop `crates/py/python/tests/ros2/` on ROS 2 Humble (6 green: init,
pub/sub, service call, topic graph, node names, endpoint info) via
`run_ros2_pytest.sh` +
rclcpp loan e2e via `run_loaned_message_test.sh` (Portable + RawSameHost +
`ZERODDS_TEST_ICEORYX=1` Iceoryx, each `can_loan=1 got=42 PASS`).

No open rmw item; all three delivery modes (`Portable`/`RawSameHost`/`Iceoryx`)
are rmw-side wired + e2e verified, and the raw readiness is event-driven (doorbell
thread on `zerodds_reader_raw_wait`, SHM futex or iceoryx2 listener) — closing the
last open delivery-modes refinement (`docs/specs/zerodds-delivery-modes-1.0.md`).
Decision records (REP-2007/2008/2009 — features live in `rclcpp` over RMW,
integrable via the `rmw_zerodds` FFI): see `ros2-rmw.open.md`.
