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
| REP-2007 type adaptation / REP-2008 accelerator loan memory | `crates/rmw-zerodds-shim/rmw_c/rmw_zerodds_ros2_ext.h` |

Implementation: `crates/ros2-rmw/` (11 modules, 81 tests green via
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

REP-2007 deliberately places the user-facing API (`rclcpp::TypeAdapter`) in
rclcpp, and in the same section names what is left to the middleware: "Perhaps
we can support a form of this feature in other languages in rcl or rmw in the
future. One challenge in doing this is that it may require **custom type
support, which may be middleware specific**." The four items below are that
custom type support. The seam to `rclcpp` is the C++ template syntax
(`TypeAdapter<Custom, Ros>`, `adapt_type::as`, `RCLCPP_USING_...`) and the
ownership mechanics over `unique_ptr`/`shared_ptr`; everything below it — wire
form, type identity, copy count — lives here.

### Defining a type adapter — rmw-side custom type support

**Spec:** REP-2007 §"Defining a Type Adapter": an adapter names `custom_type`,
`ros_message_type` and the conversion functions. §"Adding this feature in
rclcpp": "it may require custom type support, which may be middleware specific".

**Repo:** `crates/rmw-zerodds-shim/rmw_c/rmw_zerodds_ros2_ext.h`
(`rmw_zerodds_type_adapter_t` — `ros_type_name`, `serialized_size`,
`serialize`, `deserialize`) + `rmw_c/rmw_zerodds.c`
(`rmw_zerodds_publisher_set_type_adapter`,
`rmw_zerodds_subscription_set_type_adapter`; adapter field in
`zerodds_pub_data_t`/`zerodds_sub_data_t`). The endpoint itself is built
unchanged from the `ros_message_type`'s rosidl type support, so the DDS topic
name, the ROS type name and the RIHS type hash stay those of the unadapted
endpoint.

**Tests:** `rmw_c/type_adaptation_test.cpp` (rclcpp e2e, bound to the rmw
handles of live rclcpp entities; the check "endpoint topic type unchanged by
adaptation" compares `get_publishers_info_by_topic` before and after binding)
via `rmw_c/run_type_adaptation_test.sh`;
`crates/rmw-zerodds-shim/tests/ros2_ext_surface.rs::every_declared_extension_symbol_is_defined`.

**Status:** done

### Type adaptation in topics

**Spec:** REP-2007 §"Type Adaptation in Topics" — an adapted publisher or
subscription exchanges the `custom_type` while the `ros_message_type` is what
appears on the wire.

**Repo:** `rmw_c/rmw_zerodds.c` (`rmw_zerodds_publish_adapted`,
`rmw_zerodds_take_adapted`, `zerodds_adapted_wire` — the 4-byte CDR_LE
encapsulation is identical to the introspection publish path). As part of this,
`rmw_take_with_info`, `rmw_take_loaned_message_with_info` and
`rmw_take_serialized_message_with_info` now fill `rmw_message_info_t`
(`zerodds_fill_message_info`: `received_timestamp`,
`reception_sequence_number`) instead of returning an all-zero struct — rclcpp
hands that struct to the adapted callback.

**Tests:** `crates/rmw-zerodds-shim/rmw_c/type_adaptation_test.cpp`:
`"2) adapted publish -> plain subscription"` (adapted publisher, plain
rclcpp subscription of the ROS message type),
`"3) plain publish -> adapted take"` (the other direction),
`"   adapted take fills rmw_message_info_t"`; driven by
`crates/rmw-zerodds-shim/rmw_c/run_type_adaptation_test.sh`.

**Status:** done

### Type adaptation in services and actions

**Spec:** REP-2007 §"Type Adaptation in Services": request and response half are
adapted independently ("either the request or the response can be adapted").
§"Type Adaptation in Actions": the same structure for goal, feedback and result.

**Repo:** `rmw_c/rmw_zerodds.c` (`rmw_zerodds_client_set_type_adapters`,
`rmw_zerodds_service_set_type_adapters`, `rmw_zerodds_send_request_adapted`,
`rmw_zerodds_take_request_adapted`, `rmw_zerodds_send_response_adapted`,
`rmw_zerodds_take_response_adapted`; the adapted payload carries the same
24-byte correlation header as the unadapted path). A ROS 2 action is composed by
`rcl` from three services (goal, cancel, result) and two topics (feedback,
status) — there is no separate action rmw API — so the service plus topic
surface covers action adaptation.

**Tests:** `crates/rmw-zerodds-shim/rmw_c/type_adaptation_test.cpp` —
`"5) adapted service request/response roundtrip"` over
`example_interfaces/srv/AddTwoInts` with `std::pair<int64_t,int64_t>` as the
request and `int64_t` as the response custom type (neither is a ROS
message); driven by `crates/rmw-zerodds-shim/rmw_c/run_type_adaptation_test.sh`.

**Status:** done

### (De)serialize instead of convert — the avoided copy

**Spec:** REP-2007 §"Comparison to ROS 1's Type Adaptation": "This feature will
support both convert and (de)serialize functions […] convert is superior for
intra-process communication and the (de)serialization functions are better for
inter-process communication." And: "An advantage of following the ROS 1 approach
is that an extra copy can be avoided; although it is likely much more
challenging to implement this feature the ROS 1 way because of the middleware."

**Repo:** `rmw_c/rmw_zerodds.c` (`zerodds_adapted_wire`): the wire buffer is
allocated to `serialized_size(custom)` and the adapter writes the CDR body
straight into it. No `ros_message_type` instance is created, so the second copy
the REP describes as avoidable is not made. The reverse direction likewise in
`rmw_zerodds_take_adapted`: the body is read straight into the `custom_type`.

**Tests:** `rmw_c/type_adaptation_test.cpp` measures three quantities: "adapted
wire bytes == rmw_serialize bytes" (`memcmp` against the `rmw_serialize` output
of the same message — wire identity is measured, not asserted by construction),
"exactly one serialization per adapted publish" (counter in the adapter's
`serialize`) and "ros_message_type never materialized (no extra copy)" (the
test's `convert_to_ros_message` counter has to stay 0). Via
`rmw_c/run_type_adaptation_test.sh`. Wire-identity regression:
`crates/rmw-zerodds-shim/tests/ros2_ext_surface.rs::adapted_publish_writes_the_same_encapsulation_header_as_plain_publish`.

**Status:** done

---

## REP-2008 Hardware Acceleration

REP-2008 assigns the layers itself: §"Kernel levels in ROS 2" puts `rmw` and the
`rmw_adapters` on level 2 and the communication middleware ("including the
communication middleware (e.g. DDS)") on level 3, and §"Methodology" step 3.2
asks to accelerate "inter-Node exchanges" and to reduce "the overhead of the
ROS 2 message-passing system across all its abstraction layers". The seam is the
kernel languages (CUDA, HLS, OpenCL) and the firmware — both of which the REP
explicitly assigns to the silicon vendor and the application architect. What
lives here is the memory provenance of the buffers that travel between the
accelerator and the wire.

### Kernel-level assignment

**Spec:** REP-2008 §"Kernel levels in ROS 2" — level 2: "we consider rclcpp,
rcl, rmw, and the corresponding rmw_adapters for each supported communication
middleware". Level 3: "Groups together all accelerators below the ROS 2 core
layers […] including the communication middleware (e.g. DDS)". And: "Hardware
acceleration solutions complying with this REP should aspire to support multiple
kernel levels in ROS 2".

**Repo:** `crates/rmw-zerodds-shim/rmw_c/rmw_zerodds_ros2_ext.h`
(`RMW_ZERODDS_ACCEL_LEVEL_1/2/3`) + `rmw_c/rmw_zerodds.c`
(`rmw_zerodds_acceleration_kernel_levels` → level 2 | level 3). Level 1 (ROS 2
applications and libraries) stays with the application.

**Tests:** `rmw_c/accel_loan_memory_test.cpp` ("REP-2008 kernel levels 2 and 3
declared") via `rmw_c/run_accel_loan_memory_test.sh`;
`crates/rmw-zerodds-shim/tests/ros2_ext_surface.rs::acceleration_kernel_levels_agree_across_header_impl_and_readme`
(header, implementation and the vendor template in the README must name the same
number).

**Status:** done

### Pillar I — extensions to the ament build system

**Spec:** REP-2008 §"Pillar I - Extensions to ament" + §Specification: "Package
maintainers are encouraged to integrate hardware acceleration in their ROS 2
packages by using the CMake hardware acceleration variables […] These CMake
macros are meant to be used by maintainers to conditionally compile their ROS 2
packages". Variables: `ROS_ACCELERATION` (`ament_hardware_acceleration`),
`ROS_VITIS` (`ament_vitis`).

**Repo:** `crates/rmw-zerodds-shim/ament/CMakeLists.txt`
(`find_package(ament_hardware_acceleration QUIET)`,
`find_package(ament_vitis QUIET)`, evaluation of `ROS_ACCELERATION`/`ROS_VITIS`,
`RMW_ZERODDS_ACCELERATION_KERNEL_LEVELS`, `RMW_ZERODDS_LOAN_ALIGNMENT=page` in
the accelerator case; installation of `rmw_zerodds_ros2_ext.h`) +
`crates/rmw-zerodds-shim/ament/package.xml` (conditional
`ament_hardware_acceleration` build dependency). Both `find_package` calls are
`QUIET`: a workspace without an accelerator configures unchanged.

**Tests:** `crates/rmw-zerodds-shim/rmw_c/accel_loan_memory_test.cpp` covers the runtime side of the same
decision: `"2) set_loan_alignment(page) accepted"`,
`"2) portable loan starts on a page boundary"` (with
`ZERODDS_LOAN_ALIGNMENT=page`) and
`"   non-power-of-two alignment rejected"`; driven by
`crates/rmw-zerodds-shim/rmw_c/run_accel_loan_memory_test.sh`.

**Status:** done

### Pillar II and Template for Vendors — declared support level

**Spec:** REP-2008 §"Pillar II - firmware": "each vendor should provide and
maintain an `acceleration_firmware_<solution>` package specialization that
delivers the corresponding artifacts" — firmware artifacts per silicon solution.
§"Template for Vendors": "Silicon vendors and solution manufacturers can help
set the expectations of the level of support their hardware acceleration
technology provides in alignment with this REP by providing a template in the
README.md files of their firmware and/or ament extensions."

**Repo:** `crates/rmw-zerodds-shim/README.md` §"Template for Vendors (REP-2008
§"Template for Vendors")" — the table the REP asks for: kernel levels served,
Pillar I consumed, Pillar II not provided (firmware is the silicon vendor's
`acceleration_firmware_<solution>` package; ZeroDDS ships no firmware
artifacts), technology agnosticism, the accelerator-facing API and the
environment switches.

**Tests:**
`crates/rmw-zerodds-shim/tests/ros2_ext_surface.rs::acceleration_kernel_levels_agree_across_header_impl_and_readme`
(the template may not diverge from header and implementation).

**Status:** done

### Methodology §3.2 — accelerator-registerable loan memory

**Spec:** REP-2008 §"Methodology for developing ROS 2 packages that integrate
hardware acceleration", step 3.2: "accelerate inter-Node exchanges and reduce
the overhead of the ROS 2 message-passing system across all its abstraction
layers. To do so, consider leveraging REP-2007 and REP-2009."

CUDA, ROCm and XRT can only DMA out of host memory they have registered
(pinned), and they pin whole pages. Without a statement from the middleware
about where a loaned message buffer lives and which page-aligned extent covers
it, an accelerated node needs its own pinned staging buffer and copies on every
publish — exactly the copy step 3.2 wants removed.

**Repo:** `crates/rmw-zerodds-shim/rmw_c/rmw_zerodds_ros2_ext.h`
(`rmw_zerodds_loan_memory_t`: `kind`, `host_ptr`, `size`, `registration_base`,
`registration_size`, `page_size`, `cross_process`, `segment_link`) +
`rmw_c/rmw_zerodds.c` (`rmw_zerodds_publisher_get_loan_memory`,
`rmw_zerodds_subscription_get_loan_memory`, `zerodds_fill_registration`,
`rmw_zerodds_publisher_set_loan_alignment`, `zerodds_default_loan_alignment`
via `ZERODDS_LOAN_ALIGNMENT`, aligned Portable loan in
`rmw_borrow_loaned_message`). `registration_base`/`registration_size` are the
arguments for `cudaHostRegister`, `hipHostRegister` or `xrt::bo`.

**Tests:** `crates/rmw-zerodds-shim/rmw_c/accel_loan_memory_test.cpp` via
`crates/rmw-zerodds-shim/rmw_c/run_accel_loan_memory_test.sh`, in both delivery modes:
`"3) reported page size == sysconf(_SC_PAGESIZE)"`,
`"3) registration extent is page aligned on both ends"`,
`"3) registration extent covers the loaned buffer"`,
`"4) reported extent is pinnable (mlock)"` — the call performs the same
kernel operation an accelerator runtime uses to pin host memory, so
pinnability is measured rather than computed —,
`"   the described buffer is the buffer that was delivered"` and, in mode
`raw-same-host`, `"5) forked child maps the segment and finds the payload"`
and `"5) subscriber loan reports the same shared segment"`.

**Status:** done

---

## REP-2009 Type Negotiation Feature

REP-2009 places the negotiation state machine itself and, in the same breath,
states what it expects of the layers below — §"Implementation in the ROS 2 Core
or in a Separate Project": "While there are some advantages to having this
project in the ROS 2 core, **all of the needed APIs are available** to make this
a separate package. Thus, in the spirit of keeping the ROS 2 core smaller, this
should be implemented in a separate project."

The rmw obligation is therefore not to run the algorithm but to make that
sentence true. The five items below are the APIs §"Negotiation Algorithm",
§"Defining Supported Message Types" and §"Renegotiating Selected Message Types"
name by hand; each is measured against a run that drives REP-2009's algorithm
over `rmw_zerodds`. The seam is the state machine itself (priority arithmetic,
user-defined selection functions, deadlock timeout) — which the REP explicitly
assigns to a separate package above rmw.

### Endpoint info by topic — `rmw_get_publishers/subscriptions_info_by_topic`

**Spec:** REP-2009 §"Negotiation Algorithm", negotiating publisher, step 1:
"Use discovery to find all the connected subscriptions that are using the user
specified negotiation topic." The rmw surface for that is
`rmw/get_topic_endpoint_info.h`: per topic, the endpoint list with node
name/namespace, type, endpoint type, 16-byte GUID and QoS (`ros2 topic info
-v`), plus `rmw_count_publishers`/`rmw_count_subscribers`.

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
`crates/rmw-zerodds-shim/src/lib.rs::tests::participant_info_roundtrips_endpoint_gids`,
`crates/rmw-zerodds-shim/src/lib.rs::tests::participant_info_roundtrips_without_endpoints`;
`rmw_c/type_negotiation_test.cpp` ("negotiation subscription found through
discovery", "endpoint info names the negotiation message type") via
`rmw_c/run_type_negotiation_test.sh`.

**Status:** done

### Supported message type identity — RIHS type hash

**Spec:** REP-2009 §"Defining Supported Message Types": "The combination of the
ROS message type and the string identifier must form a unique tuple, since
multiple supported message types may be sent with the same ROS 2 message type."
The rmw half of that tuple is the type on the wire: type name and RIHS type hash
per endpoint (the string identifier lives in the negotiation payload, see the
next item).

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs::rmw_zerodds_compute_type_hash`
(RIHS SHA-256) + the endpoint info path above.

**Tests:** `type_hash_sha256_is_deterministic` + 2 more shim units;
`test_rclpy_endpoint_info_by_topic` (Humble);
`rmw_c/type_negotiation_test.cpp` ("discovery reports the right ROS type per
selected type" — two simultaneously selected types on two topics, each with its
own ROS type in discovery).

**Status:** done

### Carrying the supported message priority map

**Spec:** REP-2009 §"Negotiation Algorithm", step 2: "Receive the supported
message priority maps from all connected subscriptions." §"Defining Negotiating
Publishers and Subscriptions": the map travels over "a negotiation topic that
will be used to negotiate the selected message types" — ordinary ROS pub/sub.
Its shape is a sequence of nested structs carrying strings (ROS type name,
string identifier, topic, priority).

**Repo:** the introspection CDR codec in `rmw_c/rmw_zerodds.c`
(`cdr_ser_msg`/`cdr_de_msg`, sequence branch over `size_function`/
`get_const_function`/`get_function`/`resize_function`, nested message via
`type_id_ == 18`) in both typesupport flavours — see "Introspection typesupport
flavours (C and C++)".

**Tests:** `rmw_c/type_negotiation_test.cpp` with
`diagnostic_msgs/msg/DiagnosticArray` — the same shape as
`negotiated_interfaces/msg/SupportedTypes` (`DiagnosticStatus[]` with three
strings and a nested `KeyValue[]`): "priority map survives
serialize/deserialize" (`rmw_serialize`/`rmw_deserialize` identity), "supported
message priority map delivered" (over the wire) and "priority map round-trips
field for field" (compared field by field). Via
`rmw_c/run_type_negotiation_test.sh`.

**Status:** done

### One regular publisher per selected message type

**Spec:** REP-2009 §"Negotiation Algorithm", negotiating publisher: "The
negotiating publisher then creates a regular publisher for each supported
message type." §"Negotiating Subscription": "The negotiating subscription will
then create exactly one subscription corresponding to the chosen type and topic
name." The endpoints therefore come into being at runtime, after the decision,
each carrying a different ROS type.

**Repo:** `rmw_c/rmw_zerodds.c` (`rmw_create_publisher`/`rmw_create_subscription`
at runtime on a live node; the type name comes per endpoint from
`zerodds_type_name` and goes into discovery and onto the wire).

**Tests:** `crates/rmw-zerodds-shim/rmw_c/type_negotiation_test.cpp`:
`"4) both selected message types deliver on their topics"`
(`std_msgs/msg/String` and `std_msgs/msg/Int32` at the same time — the case
REP-2009 draws as `N_1(x, y)` to `N_2(x)` and `N_3(y)`),
`"4) discovery reports the right ROS type per selected type"`,
`"5) a regular publisher per selected message type"`,
`"5) the selected type appears in the graph with its ROS type"`; driven by
`crates/rmw-zerodds-shim/rmw_c/run_type_negotiation_test.sh`.

**Status:** done

### Renegotiation — endpoints leave and are created again

**Spec:** REP-2009 §"Renegotiating Selected Message Types": "If the selected
messages must change, the negotiating publisher destroys its publishers. The
negotiating publisher will then notify its subscriptions of the new selected
message types on the user specified negotiation topic and create new publishers
for the selected message types." A destroyed endpoint therefore has to leave the
graph at once, or the next round computes with a peer that no longer exists.

**Repo:** `crates/zerodds-c-api/src/lib.rs` (`zerodds_writer_destroy` /
`zerodds_reader_destroy` now call
`DcpsRuntime::remove_user_writer`/`remove_user_reader`) +
`crates/dcps/src/runtime.rs` (both methods were `pub(crate)` and thus reachable
only through the DCPS DataWriter, not through the runtime FFI path rmw uses;
they take the endpoint out of the local registry, rebuild the intra-runtime
routes, stop announcing it and send an SEDP dispose).

Previously a publisher destroyed through rmw stayed in the graph forever:
`rmw_count_publishers` and `rmw_get_publishers_info_by_topic` kept reporting it,
and a remote peer waited for the liveliness timeout instead of unmatching at
once.

**Tests:** `rmw_c/type_negotiation_test.cpp`: "destroyed publishers leave the
graph" (`rmw_destroy_publisher` on the rmw handle, then `count_publishers == 0`
and empty endpoint info), "discovery reports the renegotiated ROS type" (the
same topic afterwards carries a different ROS type) and "the renegotiated
selected type delivers". Via `rmw_c/run_type_negotiation_test.sh`.

**Status:** done

### The network change as a trigger — graph guard condition

**Spec:** REP-2009 §"Renegotiating Selected Message Types": "In the case that
the ROS 2 network is changing, the negotiating publisher may need to renegotiate
its selected message types." Together with step 1 ("Use discovery to find all
the connected subscriptions") that means the graph has to *announce* the change,
not merely be pollable. rcl surfaces it as the node's graph guard condition.

**Repo:** `rmw_c/rmw_zerodds.c` (`zerodds_refresh_graph_guards`,
`zerodds_fp_cb`, `zerodds_node_register`/`_unregister`, `graph_fp` in
`zerodds_node_data_t`): for every graph guard condition actually present in a
wait set, `rmw_wait` folds all discovered publications and subscriptions of that
node into a fingerprint and raises the flag when it changes. A node nobody
watches costs nothing.

Alongside it, the guard condition evaluation itself: rcl hands `rmw_wait` the
guard condition's **`data` pointer** in
`rmw_guard_conditions_t.guard_conditions[i]`, not the `rmw_guard_condition_t`
(`rcl_wait_set_add_guard_condition`: `RMWStorage[n] = rmw_handle->data`). The
shim read the entry as a struct and compared its `implementation_identifier` —
the comparison never matched, so **no** guard condition ever reported readiness:
neither the graph guard condition, nor the executor's interrupt guard condition,
nor a user-created one. The entry is now read as what it is.

Also here: `rmw_wait` returned `RMW_RET_TIMEOUT` on an infinite timeout when a
wake-up arrived with nothing ready. rcl's graph listener treats that as a fatal
error and aborts the process with "rcl_wait unexpectedly timed out"; a wait
without a deadline cannot time out and now reports `RMW_RET_OK`.

**Tests:** `crates/rmw-zerodds-shim/rmw_c/type_negotiation_test.cpp` —
`"6) the graph change raised the node graph guard"`: the graph event is armed
before the renegotiation, the wait path blocks on the guard condition, and
the event must be set afterwards. Driven by
`crates/rmw-zerodds-shim/rmw_c/run_type_negotiation_test.sh`.

**Status:** done

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
`crates/ros2-rmw/src/rmw_qos_mapping.rs::tests::rmw_system_default_passes_through_as_sentinel`,
`crates/ros2-rmw/src/rmw_qos_mapping.rs::tests::rmw_unknown_passes_through_as_sentinel`,
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

**Repo:** `rmw_zerodds.c` (service typesupport introspection, CDR
`[encap][body]`, rcl request ids) + `src/services.rs` (topics `rq/…Request`/
`rr/…Reply`, types `…_Request_`/`_Response_`, Cyclone form and
related-identity form, waiting up to 100 ms for the reply reader).

**Tests:** `crates/rmw-zerodds-shim/src/lib.rs::tests::{a_service_call_between_two_runtimes_correlates_by_the_related_identity,
two_clients_in_one_runtime_each_get_their_own_reply}`;
rclpy `test_rclpy_service_call_roundtrip` (AddTwoInts 41+1=42);
`crates/rmw-zerodds-shim/rmw_c/run_ros2_services_test.sh` (Linux test host, against
rmw_cyclonedds_cpp and rmw_fastrtps_cpp in both directions).

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

### Service graph and names by node

**Spec:** `rmw/rmw.h` `rmw_get_service_names_and_types`,
`rmw_count_services/clients`, `rmw_get_{publisher,subscriber,service,client}_names_and_types_by_node`,
`no_demangle` of `rmw_get_topic_names_and_types`,
`rmw_qos_profile_check_compatible`.

**Repo:** `rmw_zerodds.c` (`zerodds_demangle_service`,
`zerodds_demangle_service_type`, `zerodds_names_and_types_by_node`: each
endpoint's node from `ros_discovery_info`, services = readers on request
topics, clients = writers on them), `rmw_zerodds_qos.cpp`
(`rmw_dds_common::qos_profile_check_compatible`, as rmw_cyclonedds_cpp);
`src/services.rs` announces service endpoints in `ros_discovery_info`.
Before, all of these returned nothing or "compatible".

**Tests:** `crates/rmw-zerodds-shim/rmw_c/run_ros2_services_test.sh`
(`ros2 service list -t` and `ros2 node info` across RMWs, Linux test host).

**Status:** done

### Guard conditions + event-driven wake

**Spec:** `rmw/rmw.h` `rmw_create_guard_condition` /
`rmw_trigger_guard_condition` / `rmw_wait`. rcl stores a guard condition's
`data` pointer in `rmw_guard_conditions_t.guard_conditions[i]`, not the
`rmw_guard_condition_t` (`rcl_wait_set_add_guard_condition`:
`RMWStorage[n] = rmw_handle->data`); after the wait rcl reads exactly that array
to decide which guard condition fired.

**Repo:** `rmw_zerodds.c` (`rmw_create/trigger_guard_condition` →
`context_notify` wakes the wait condvar; `rmw_wait` reads the array entries as
`unsigned int *` flags, consumes a raised flag and nulls the rest).

Previously `rmw_wait` read the entry as an `rmw_guard_condition_t *` and checked
its `implementation_identifier`. The comparison never matched, so no guard
condition ever reported readiness — neither the executor's interrupt guard
condition, nor a node's graph guard condition, nor a user-created one. Second
defect on the same path: a wait with an infinite timeout returned
`RMW_RET_TIMEOUT` on a wake-up with nothing ready; rcl's graph listener treats
that as fatal and aborts the process with "rcl_wait unexpectedly timed out".

**Tests:** `crates/rmw-zerodds-shim/rmw_c/type_negotiation_test.cpp` —
`"6) the graph change raised the node graph guard"`: the event is set only
when the guard condition really reports readiness; driven by
`crates/rmw-zerodds-shim/rmw_c/run_type_negotiation_test.sh`. The `rmw_wait` path itself:
`crates/rmw-zerodds-shim/src/lib.rs::tests::wait_notify_blocks_then_wakes_on_notify`,
`::event_driven_wait_roundtrip_inprocess`, `::wait_returns_invalid_for_null`,
`::has_data_and_context_block_null_safe`.

**Status:** done

### `rmw_get_node_names` + `_with_enclaves`

**Spec:** `rmw/rmw.h` `rmw_get_node_names[_with_enclaves]`.

**Repo:** `rmw_zerodds.c` (`rmw_get_node_names`, accumulator) + `src/lib.rs`
(`NodeGraph`, `encode/decode_participant_info`, `discovery_on_data`,
`rmw_zerodds_for_each_node`) — each context publishes its
`ParticipantEntitiesInfo` on `ros_discovery_info` (hand-encoded XCDR1) and
aggregates local + remote nodes.

**Tests:** rclpy `test_rclpy_node_names_graph`.

**Status:** done — `ros_discovery_info` as in rmw_dds_common: NoKey,
RELIABLE, TRANSIENT_LOCAL, KEEP_LAST(1) (`src/lib.rs`, discovery QoS); a
process started later receives each participant's latest state.

### `on_new_message/request/response` callbacks (events executor)

**Spec:** `rmw/rmw.h` `rmw_*_set_on_new_*_callback`.

**Repo:** `rmw_zerodds.c` (3 setters → bridge) + `src/lib.rs`
(`SubInbox.event`, `inbox_set_event`, invoked in `subscription_on_data`).

**Tests:** `rmw-zerodds-shim::tests::event_callback_fires_on_arrival`.

**Status:** done

### `rmw_get_serialized_message_size`

**Spec:** `rmw/rmw.h` `rmw_get_serialized_message_size`.

**Repo:** `crates/rmw-zerodds-shim/rmw_c/rmw_zerodds.c`
(`rmw_get_serialized_message_size`, `zerodds_cdr_max_msg`,
`zerodds_cdr_max_elem` — introspection size walk, conservative upper
bound; strings/sequences capped).

**Tests:** `crates/rmw-zerodds-shim/tests/ros2_ext_surface.rs::the_serialized_size_walk_covers_the_header_and_every_member_kind`
— checks that the returned value adds the four-byte encapsulation header,
that every member kind (primitive, string, nested, scalar, fixed array,
sequence) has a branch in the walk, and that `ZERODDS_STR_CAP`/
`ZERODDS_SEQ_CAP` reserve room rather than nothing. No gate.

**Status:** partial — the walk is implemented and structurally checked.
What is missing is the behavioural check "the returned value really is
≥ the length `rmw_serialize` writes"; that needs the rosidl introspection
type supports and hence a ROS 2 host. Tracked as its own item below.

### `rmw_get_serialized_message_size` — bound check on a ROS 2 host

**Spec:** `rmw/rmw.h` `rmw_get_serialized_message_size` — the value handed
back must suffice for the pre-allocation.

**Repo:** —

**Tests:** —

**Status:** open — no test case holds
`rmw_get_serialized_message_size` for a real message against the length
`rmw_serialize` writes. That needs a ROS 2 host; the case belongs beside
the existing rclcpp e2e tests in `crates/rmw-zerodds-shim/rmw_c/`.

### Typed message loaning (`rmw_borrow/publish/take_loaned_message`)

**Spec:** `rmw/rmw.h` loaned-message API + `can_loan_messages`. Delivery shape:
`docs/specs/zerodds-delivery-modes-1.0.md` (modes `Portable`/`RawSameHost`/`Iceoryx`).

**Repo:** `rmw_c/rmw_zerodds.c` (`rmw_borrow/publish/take_loaned_message`,
`rmw_return_loaned_message_from_publisher/subscription`, `can_loan_messages`
= fixed-POD from rosidl introspection) + the SHM bridge in
`crates/rmw-zerodds-shim/src/lib.rs`
(`rmw_zerodds_publisher_enable_raw_loan`/`_loan`/`_commit`/`_discard`,
`rmw_zerodds_subscription_enable_shm`/`_take_shm`/`_has_shm_data`/`_release_shm`),
feature `flatdata-loan` (default).

**Tests:** `crates/rmw-zerodds-shim/src/lib.rs::tests::loaning_null_args_rejected`,
`crates/rmw-zerodds-shim/src/lib.rs::tests::return_loan_null_args_rejected`
(the argument contract of the loan ABI, no gate, they run in the default
run). Behaviour in both delivery modes:
`crates/rmw-zerodds-shim/rmw_c/loaned_message_test.cpp`, started by
`crates/rmw-zerodds-shim/rmw_c/run_loaned_message_test.sh` — an rclcpp e2e
that runs only on a host with ROS 2 and is not part of the cargo default
run.

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

### Introspection typesupport flavours (C and C++)

**Spec:** `rosidl` ships two introspection typesupports with identical
`MessageMembers`/`ServiceMembers` layout but different runtime types for the
variable-length members: `rosidl_typesupport_introspection_c`
(`rosidl_runtime_c__String`/`__U16String`, resolved by `rclpy`) and
`rosidl_typesupport_introspection_cpp` (`std::string`/`std::u16string`, resolved
by `rclcpp`). An rmw has to serve both.

**Repo:** `rmw_c/rmw_zerodds.c` (`zerodds_introspect_ex`,
`zerodds_service_introspect_ex` report the flavour as a flag; the `cpp` field in
`crates/rmw-zerodds-shim/rmw_c/rmw_zerodds.c::cdr_t` and
`crates/rmw-zerodds-shim/rmw_c/rmw_zerodds.c::rdr_t` carries it through
serialization and deserialization; publisher,
subscription, client and service store it in `members_cpp`) +
`rmw_c/rmw_zerodds_cxx.cpp` (`zerodds_cxx_string_data/_assign`,
`zerodds_cxx_u16string_data/_assign` — the C translation unit cannot touch
`std::string`) + `rmw_c/build_librmw_zerodds_cpp.sh` (compiles and links the C++
translation unit, `-lstdc++`).

Previously the C++ flavour was handled like the C one. A `std::string` member
ended up in `rosidl_runtime_c__String__assignn`, which reallocs the
`std::string`'s SSO buffer as if it were a heap pointer — `realloc(): invalid
pointer`, process abort on the first `std_msgs/msg/String` over `rclcpp`.
`zerodds_service_introspect` also queried only the C flavour, so
`rmw_create_client`/`rmw_create_service` returned NULL for `rclcpp` services.

**Tests:** `crates/rmw-zerodds-shim/rmw_c/type_adaptation_test.cpp` —
`"2) adapted publish -> plain subscription"`,
`"3) plain publish -> adapted take"`,
`"5) adapted service request/response roundtrip"` (`std_msgs/msg/String`
through `rclcpp` publisher and subscription in both directions,
`example_interfaces/srv/AddTwoInts` through `rclcpp` client and service),
driven by `crates/rmw-zerodds-shim/rmw_c/run_type_adaptation_test.sh`. Regression of the C flavour:
`crates/py/python/tests/ros2/test_rmw_zerodds_interop.py::test_rclpy_publish_subscribe_string_roundtrip`
and `::test_rclpy_service_call_roundtrip` via `crates/rmw-zerodds-shim/rmw_c/run_ros2_pytest.sh`.

**Status:** done

---

## ROS 2 Jazzy Dynamic Type Support (`rosidl_dynamic_typesupport`)

### Interface table + reflective mapping

**Spec:** `rosidl_dynamic_typesupport/api/serialization_support_interface.h`
(ROS 2 Jazzy) — `rosidl_dynamic_typesupport_serialization_support_interface_t`,
192 function pointers: dynamic-type builder (every primitive/string kind ×
scalar/array/bounded/unbounded sequence, nested members through both
`dynamic_type_impl_t` and `dynamic_type_builder_impl_t`), dynamic data
(get/set/insert for every kind, sequence operations, nested values, loan,
serialization). Implemented by `rosidl_dynamic_typesupport_fastrtps` for
Fast DDS and by this repo for ZeroDDS — the header's own description:
"Downstream middlewares should populate this interface as appropriate."

**Repo:** `crates/rmw-zerodds-shim/src/dynamic.rs` — every interface
function maps onto `zerodds_types::dynamic::{DynamicTypeBuilder,
DynamicType, DynamicData}`, the same reflective type family
`zerodds-dcps`'s `SerializedTypeSupport` uses for runtime-described
samples; serialization/deserialization goes through
`zerodds_wire_codec::{encode,decode}`, the same codec path every other
`DynamicData` consumer in the repo uses.
`crates/rmw-zerodds-shim/rmw_c/rmw_zerodds_dynamic.cpp` populates the 192
function pointers; the ~90 `extern "C"` entry points behind them are
declared by hand (cbindgen does not expand `macro_rules!`, see the
file's own comment), not taken from the generated `include/rmw_zerodds.h`
— the same style `rmw_c/rmw_zerodds.c` already uses for its own `extern`
declarations.

Since `dynamic_type_builder_init` cannot assume any shared model state
across calls (each call gets a fresh `..._impl_t` with no parent
reference), every builder owns its own private
`zerodds_types::dynamic::DynamicTypeBuilder`/`Ast`; a nested type is
grafted — re-declared node by node, memoised — into the target `Ast`
when composed (`add_complex_member[_builder]`), rather than sharing one
model. Reasoning in `crates/rmw-zerodds-shim/src/dynamic.rs`'s module comment. The graft copies
every facet a member or the struct type itself carries — `@key`,
`@optional`, `@must_understand`, `@external`, `@try_construct`, the
effective `@id`, `@default`, and `@nested`/`@autoid(HASH)` plus
extensibility on the type — not only the type and the effective id: an
earlier version silently dropped every one of these but the id, so a
composed type lost its key, its `@optional`, its `@default` and more,
with nothing reporting it. A `@default` value survives the graft
through its text, not its `Id<ConstExprNode>` — that is not portable
between two `Ast` instances — via
`zerodds_types::dynamic::default_expr_text`, the inverse of the same
parser that reads a `@default` text against the member's type in the
first place.

**Tests:** `cargo test -p rmw-zerodds-shim --lib dynamic::tests::` (15
green): every primitive/string kind round-trips through get/set/insert
(`every_primitive_kind_round_trips`), nested structs + sequences
(`nested_struct_and_sequence_round_trip`), clone/equality on both type
and data (`clone_and_equals_agree`), loan/return-loan writing back at the
loaned origin (`a_loan_writes_back_at_its_own_origin`), a bounded string
past its bound is refused rather than truncated
(`a_bound_violation_is_refused_not_truncated`), the representation header
octets match DDSI-RTPS 2.5 §10.5
(`representation_of_matches_the_rtps_table`) — and, as evidence for
"byte-identical to the static typesupport's CDR for the same message":
`a_mixed_alignment_struct_matches_plain_cdr_byte_for_byte` compares
`dynamic_data_serialize`'s output for a `bool`+`int32`+`float64` struct,
byte for byte, against a by-hand plain-CDR encoding (alignment relative
to the body start after the 4-byte encapsulation header, exactly as
`rmw_zerodds.c`'s own `cdr_align`/`rdr_align` compute it — `(len - 4) %
a`). For the graft fix specifically:
`a_grafted_members_facets_survive_and_match_idl_encoding` builds a
nested struct type with a `@key`, a `@default(42)` and an `@optional`
member directly through `DynamicTypeBuilder`/`NewMember` (the C
interface itself has no parameter for any of the three), composes it
into an outer type through `zerodds_dyn_builder_add_nested_member`,
reads the surviving `@default` back after
`dynamic_data_init_from_dynamic_type`, and compares the serialized
bytes octet for octet against the same declaration written in IDL and
encoded directly with `zerodds_wire_codec::encode` — exactly where a
dropped `@key` or `@optional` would show: §7.4.1.1's presence flag
ahead of an `@optional` member shifts every byte after it.

**Status:** done

### RMW integration (`rmw_serialization_support_init`, `rmw_take_dynamic_message[_with_info]`)

**Spec:** `rmw/dynamic_message_type_support.h` (ROS 2 Jazzy) — the three
rmw entry points, previously answered with `RMW_RET_UNSUPPORTED` (as
`rmw_cyclonedds_cpp` does), now really implemented.

**Repo:** `crates/rmw-zerodds-shim/rmw_c/rmw_zerodds_dynamic.cpp` —
`rmw_serialization_support_init` checks `serialization_lib_name` against
its own identifier `"rmw_zerodds_cpp"` (RMW_RET_INCORRECT_RMW_IMPLEMENTATION
on a mismatch) and calls
`rosidl_dynamic_typesupport_serialization_support_init` with the table
built above; `rmw_take_dynamic_message[_with_info]` uses the same raw
sample path the static introspection codec uses
(`rmw_zerodds_take`/`_with_origin`, `librmw_zerodds.so`) and hands the
CDR stream — encapsulation header included — to
`dynamic_data_deserialize`, which reads the representation off the
header itself rather than guessing it.
`crates/rmw-zerodds-shim/ament/CMakeLists.txt` and
`rmw_c/build_librmw_zerodds_cpp.sh` build the new file in;
`find_package(rosidl_dynamic_typesupport QUIET)` (CMake) and
`<depend condition="$ROS_DISTRO != 'humble'">` (`package.xml`) keep
Humble building unchanged.

A member's `default_value` covers array/sequence shapes too, not only
scalars: XTypes' `@default` (§7.3.4.5.4) has no collection form, so a
fixed array's/bounded sequence's/unbounded sequence's default stays out
of the model entirely and is parsed
(`crates/rmw-zerodds-shim/src/dynamic/collection_default.rs`) straight
into `Vec<DynamicData>` elements, kept next to the type
(`ZddsDynBuilder`/`ZddsDynType`/`ZddsDynData`) and applied at
`dynamic_data_init_from_dynamic_type*` and at
`dynamic_data_clear_value`/`clear_all_values`/`clear_nonkey_values`. The
text is `rosidl_adapter`'s `repr(tuple(value))` inside an IDL string
literal (`@default(value="(1, 2, 3)")`) — the module note gives the
element grammar (bool `True`/`False`, a plain decimal integer for every
integer kind including `byte`/`char`, a decimal or scientific float or
`inf`/`-inf`/`nan`, a quoted backslash-escaped string). A text that does
not parse, does not fit its element kind's range, runs past a bounded
string's own bound, or makes the wrong number of elements is refused,
never silently shortened, clamped or padded.

**Tests:** Syntax check against the copied ROS 2 Jazzy headers (`rmw`,
`rcutils`, `rosidl_dynamic_typesupport`, `rosidl_runtime_c`,
`rosidl_typesupport_introspection_{c,cpp}`, plus a hand-written stand-in
for `rclcpp`/`test_msgs` covering every symbol this file calls) via
`c++ -std=c++17 -fsyntax-only -Wall -Wextra -Wpedantic -Wshadow
-Wconversion` — 0 errors, 0 warnings; every `rosidl_dynamic_typesupport`
function name this file calls (63 in all) checked to exist, by name,
in those headers.
`crates/rmw-zerodds-shim/rmw_c/dynamic_typesupport_test.cpp` +
`run_dynamic_typesupport_test.sh`: `std_msgs/msg/String` and
`test_msgs/msg/BasicTypes` (every primitive kind) through a real
publisher, compared between a statically typed `rclcpp::Subscription`
and a `rosidl_dynamic_typesupport_dynamic_data_t` taken through
`rmw_take_dynamic_message` on the same topic; `test_msgs/msg/Arrays` and
`test_msgs/msg/BoundedSequences` the same way, every primitive/string
kind again as a fixed array/bounded sequence, once plain and once with
a `default_value`, plus the three nested-message members and the
trailing alignment scalar the real messages also declare. Run against
ROS 2 Jazzy (RoboStack) on the Linux test host: 17 of 17 checks pass —
before the member-default, `long double` round-trip, and
Arrays/BoundedSequences checks added since; not yet re-measured on that
host.

`crates/rmw-zerodds-shim/src/dynamic/collection_default.rs`: 18 unit
tests for the parser (empty/singleton/multi-element tuples, every
integer kind's own range, `inf`/`-inf`/`nan`, quoting and escaping on
either side, a comma inside a quoted string, the wrong element count
for a fixed array and past a bounded sequence's bound — each refused).
`crates/rmw-zerodds-shim/src/dynamic.rs`: a struct with one member of
each collection shape (a fixed array, a bounded sequence, an unbounded
sequence), each with its own `default_value`, proves
`dynamic_data_init_from_dynamic_type`/`clear_value`/`clear_all_values`
all apply a collection member's own recorded default rather than Table
9's empty collection, plus two refusal tests (wrong length for a fixed
array, past a bounded sequence's bound).

**Status:** partial — the interface table and the RMW wiring run
(`run_dynamic_typesupport_test.sh` against ROS 2 Jazzy; last measured
17/17 pass, before the checks named above). Two named, deliberate
gaps, none of them
hidden: (1) `dynamic_data_serialize` has no negotiated
`DataRepresentation` QoS to consult — the interface gives the call no
writer/QoS context at all — and so always writes `@final`,
little-endian, XCDR1 (PLAIN_CDR1), matching the framing choice the
static introspection codec in `rmw_zerodds.c` already makes. (2) `long
double`/`float128` is bridged only for the x86-64 80-bit-extended and
the aarch64 true-binary128 memory layouts
(`Float128::{to,from}_x87_padded_bytes`/`{to,from}_le_bytes`); 32-bit
ARM and PowerPC's double-double `long double` are refused with
RCUTILS_RET_UNSUPPORTED rather than guessed.

---

## Audit status

38 done / 2 partial / 1 open / 1 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-ros2-rmw` (81 green) +
`cargo test -p rmw-zerodds-shim` (45 green: 24 unit, 12 `shim_cli_e2e`,
5 CLI binary, 4 `ros2_ext_surface`) +
rclpy interop `crates/py/python/tests/ros2/` on ROS 2 Humble (6 green: init,
pub/sub, service call, topic graph, node names, endpoint info) via
`run_ros2_pytest.sh` +
rclcpp loan e2e via `run_loaned_message_test.sh` (Portable + RawSameHost, each
`can_loan=1 got=42 PASS`) +
REP-2007 e2e via `run_type_adaptation_test.sh` (13 checks PASS, 0 failures) +
REP-2008 e2e via `run_accel_loan_memory_test.sh` (Portable 15 checks PASS,
RawSameHost 16 checks PASS, 0 failures each) +
REP-2009 e2e via `run_type_negotiation_test.sh` (16 checks PASS, 0 failures).

All ROS 2 runs on the Linux test host (ROS 2 Humble via RoboStack/micromamba).

Open items: see `ros2-rmw.open.en.md`. Decision records: none.
