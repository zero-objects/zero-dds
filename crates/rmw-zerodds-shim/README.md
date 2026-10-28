# rmw-zerodds-shim — ROS 2 RMW implementation

Provides `librmw_zerodds.{so,dylib,dll}` so that rclcpp/rclpy can use
ZeroDDS as the RMW backend via
`RMW_IMPLEMENTATION=rmw_zerodds_cpp`.

## Architecture

```
  rclcpp / rclpy / rclrs
      │  rmw API (REP-2007)
      ▼
  librmw_zerodds.{so,dylib,dll}    ◀── this crate
      │  ZeroDDS C-API (zerodds.h)
      ▼
  libzerodds.{so,dylib,dll}        (crates/zerodds-c-api)
      │  RTPS over UDP/TCP/SHM/UDS
      ▼
  Wire
```

The REP-2007 mapping (topic mangling, QoS profiles, identifier constraints)
comes from `crates/ros2-rmw`. The wire path is the ZeroDDS DCPS runtime
via `zerodds-c-api`. Discovery runs automatically via SPDP/SEDP.

## Capability status

**Implemented and verified** (rclpy interop pytest on ROS 2 Humble,
`crates/py/python/tests/ros2/`):

* pub-sub pipeline + initialization (init/node/publisher/subscription).
* **Event-driven wait** — reader data listener → per-subscription inbox →
  context condvar; `rmw_wait` parks on the condvar (no spin, no fixed-tick
  poll), reports subscriptions/services/clients/guards ready.
* **Cross-vendor topics** — with `rmw_cyclonedds_cpp` and `rmw_fastrtps_cpp`
  in both directions, measured with `rmw_c/run_qos_translation_test.sh`
  (`RMW_PUB`/`RMW_SUB`) and `ros2 topic pub`/`echo`: ROS DDS type names
  (`std_msgs::msg::dds_::Int32_`), NoKey endpoints, one encapsulation header
  on the wire, `ros_discovery_info` NoKey + RELIABLE + TRANSIENT_LOCAL.
* **QoS** — every field of `rmw_qos_profile_t` reaches the endpoint
  (reliability, durability, history + depth, deadline, lifespan, liveliness,
  `avoid_ros_namespace_conventions`); `rmw_*_get_actual_qos` reports what was
  applied; `rmw_publisher_assert_liveliness` and `wait_for_all_acked` act.
* **Services** — request/reply between ZeroDDS nodes and with
  `rmw_cyclonedds_cpp` and `rmw_fastrtps_cpp`, server on either side, each
  peer correlated in its own form; `rmw_service_server_is_available`.
  Measured with `rmw_c/run_ros2_services_test.sh` (AddTwoInts, graph
  queries). Cyclone and Fast DDS do not exchange service calls with each
  other; the script reports that pair without judging it.
* **Serialize / serialized publish-take** — introspection CDR; the rosbag2 path.
* **Topic-level graph** — `rmw_get_topic_names_and_types` +
  `rmw_count_publishers/subscribers` from local user endpoints + SEDP discovery.
* **Node graph** — `rmw_get_node_names[_with_enclaves]` via `ros_discovery_info`
  (`ParticipantEntitiesInfo` publish/subscribe + aggregation).
* **Event callbacks** — `rmw_*_set_on_new_*_callback` (rclcpp EventsExecutor),
  fired from the reader data listener.
* `rmw_get_serialized_message_size` (introspection size walk), type hash,
  guard conditions.
* **Typed message loaning** (`rmw_borrow/publish/take_loaned_message`,
  `can_loan_messages` for fixed-POD), in two delivery modes selected by the
  runtime setting `delivery.mode` (env `ZERODDS_DELIVERY_MODE`, or a
  configuration file; `docs/specs/zerodds-delivery-modes-1.0.md`):
  * `Portable` (default): rclcpp hands a typed struct, publish serializes
    struct→CDR over RTPS — interop-safe, same-host SHM acceleration transparent.
  * `RawSameHost` (true zero-copy / zero-serialize, same-host only): borrow → a
    pointer into the POSIX SHM slot (the user writes the struct directly);
    commit finalizes in place with no serialization and no wire (no double
    delivery); reader takes the slot zero-copy (`take_loaned`) or struct-memcpy
    (`rmw_take`).
  * `Iceoryx` (same-host cross-stack, shim feature `delivery-iceoryx`): the same
    loan/commit/take path bound to a topic-derived iceoryx2 service; without the
    feature `ZERODDS_DELIVERY_MODE=iceoryx` degrades to `Portable`.

  Readiness for the raw modes prefetches one sample into a pending buffer
  (iceoryx2's `take` is a destructive FIFO receive, so it must not be consumed by
  a peek). Verified by the rclcpp e2e in all three modes
  (`rmw_c/run_loaned_message_test.sh`, `ZERODDS_TEST_ICEORYX=1` for iceoryx;
  each `can_loan=1 got=42 PASS`).

**Follow-up (runtime-led, `docs/specs/zerodds-delivery-modes-1.0.md` §11 — not an
rmw gap):** fold the flatdata cross-process futex (already in the backend) into
the `rmw_wait` condvar via a c-api notify getter — today raw readiness is a
prefetch checked per wait wake (latency- rather than event-driven).

## Build

### Standalone (without ROS)

```bash
cargo build -p rmw-zerodds-shim --release
ls target/release/librmw_zerodds.*
ls crates/rmw-zerodds-shim/include/rmw_zerodds.h
```

### In a ROS-2 workspace

```bash
# 1) Rust build in the ZeroDDS repo (the DDS bridge and the C API)
cd ~/zerodds
cargo build -p rmw-zerodds-shim -p zerodds-c-api --release

# 2) the ament package rmw_zerodds_cpp in the ROS-2 workspace: it builds the
#    C layer, installs librmw_zerodds_cpp with the two libraries it loads, and
#    registers the implementation (run_ament_package_test.sh checks this).
mkdir -p ~/ros2_ws/src/rmw_zerodds_cpp
cp -r crates/rmw-zerodds-shim/ament/* ~/ros2_ws/src/rmw_zerodds_cpp/
cd ~/ros2_ws
ZERODDS_HOME=$HOME/zerodds \
  colcon build --packages-select rmw_zerodds_cpp
```

### Usage

```bash
. ~/ros2_ws/install/setup.bash
RMW_IMPLEMENTATION=rmw_zerodds_cpp \
  ros2 topic pub /chatter std_msgs/String "data: hello"

# In a second terminal:
RMW_IMPLEMENTATION=rmw_zerodds_cpp \
  ros2 topic echo /chatter std_msgs/String
```

## Distro-Targets

* **ROS 2 Humble** — ament_cmake 1.5.x (Ubuntu 22.04 LTS)
* **ROS 2 Iron** — ament_cmake 2.x (Ubuntu 22.04 LTS)
* **ROS 2 Jazzy** — ament_cmake 2.x (Ubuntu 24.04 LTS)

Measured on Humble. `ci/jobs/rmw-distro-build.yml` defines builds for all
three, but no pipeline includes the file; Iron and Jazzy are not measured.

## Symbols

The Rust crate exports `rmw_zerodds_*`-prefixed bridge functions (the
simplified DDS byte path). The C translation unit `rmw_c/rmw_zerodds.c` compiles
against the real Humble rmw headers and exports the genuine **unprefixed**
`rmw_*` ABI that stock rclpy/rclcpp load, bridging onto the prefixed functions.

`rmw_init_publisher_allocation`, `rmw_fini_publisher_allocation`,
`rmw_init_subscription_allocation` and `rmw_fini_subscription_allocation`
return `RMW_RET_UNSUPPORTED` (nothing is pre-allocated). Other entry points
return it only for a case they do not serve, with an error message: an event
type the entity does not have, a loan on a type that cannot be loaned, a type
adapter on the raw same-host path, loan memory for a type without a fixed
size. Implemented:

| Implemented (`rmw_*` ABI) |
| ------------------------- |
| init/shutdown/context-fini, node, guard conditions |
| publisher/subscription + `rmw_publish`/`rmw_take` |
| `rmw_borrow/publish/take_loaned_message` + `can_loan_messages` (Portable + RawSameHost) |
| `rmw_wait` (event-driven), wait-sets |
| client/service + send/take request/response |
| `rmw_serialize`/`rmw_deserialize`, serialized pub/take |
| `rmw_get_topic_names_and_types`, `rmw_count_publishers/subscribers` |
| `rmw_get_node_names`, `on_new_*` callbacks, `rmw_get_serialized_message_size` |

## SpatialDDS <-> ROS 2 content bridge (`ros2.spatial_bridge`, C2(6))

`zerodds-spatial-ros2` converts SpatialDDS types (`zerodds-spatial-dds`,
SpatialDDS 1.6) to and from ROS 2 `common_interfaces` messages
(`geometry_msgs/PoseStamped`, `geometry_msgs/TransformStamped`,
`geographic_msgs/GeoPoseStamped`, `vision_msgs/Detection3D[Array]`,
`sensor_msgs/Imu`), but nothing called it — this RMW does purely
transport-level, message-content-agnostic bridging. `src/spatial_bridge.rs`
is the missing call: an opt-in content-translation daemon, on its **own**
`DomainParticipant` (not the RMW's own runtime, not `rmw_c/`), configured in
the `ros2.spatial_bridge` section of the runtime configuration
(`man/man5/zerodds-ros2.yaml.5`, `packaging/linux/configs/ros2.yaml.example`):

```yaml
ros2:
  spatial_bridge:
    enabled: true
    mappings:
      - message: pose_stamped        # pose_stamped | transform_stamped |
        spatial_topic: sdds/default/FramedPose  # geo_pose_stamped | detection3d |
        ros_topic: /robot1/pose       # detection3d_array | imu
        direction: both               # dds_to_ros | ros_to_dds | both (default)
```

Every field a conversion drops (or defaults, coming back) — `spatial_ros2::
convert`'s own field-fidelity doc comment names them per message kind — is
reported once per `(topic, direction)` as an observability event
(`diagnostics.observability`, event name `spatial_bridge.field_fidelity`),
never silently.

Tested in-process (`tests/spatial_bridge_e2e.rs`): a SpatialDDS writer, the
bridge, and a plain DCPS reader of the ROS type share one participant (the
same-runtime direct channel handoff `zerodds-dcps`'s `ordered_access_e2e.rs`
uses, for the same reason — no UDP loopback, no platform gate), and the
reverse direction the same way. That proves the conversion and the
config/observability wiring, not interop with an independently started ROS 2
node. That is measured across processes (Linux test host, ROS 2 Jazzy, the
`rmw-spatial-bridge` example of `zerodds-examples`: an rclpy node with this
RMW and a SpatialDDS program on the wire): SpatialDDS → ROS 59 of 60 poses,
ROS → SpatialDDS 60 of 60, values intact, in two runs. Before, the ROS node
received the right number of poses with wrong values, and nothing arrived in
the other direction: the wrappers declared `@final` over the `@appendable`
generated codecs, and had no XCDR1 form, the one ROS 2 middlewares write
(`tests/spatial_bridge_wire.rs`). The ROS-side writer offers XCDR1; the
SpatialDDS side stays XCDR2, the only form its codecs have.

Both bridge writers offer RELIABLE + TRANSIENT_LOCAL + KEEP_LAST(10), and
both bridge readers request VOLATILE (DDS 1.4 §2.2.3.4, and §2.2.3 table
"QoS compatibility" for the compatibility rule). TRANSIENT_LOCAL is what a
sample needs that the pump forwarded before its own writer had discovered
the reader on the far side: a VOLATILE writer keeps no sample for a reader
that matches later. Measured against a ROS 2 Jazzy node with
`rmw_zerodds_cpp` (Linux test host, the `rmw-spatial-bridge` example), that first
pose was lost in 59 of 60 `ros_to_dds` runs (one run in three) and 57 of
58 `dds_to_ros` runs (one in 13). What a reader on a bridged topic sees
depends on what it asks for: a TRANSIENT_LOCAL reader joining late is
served the writer's last ten samples, a VOLATILE reader — the ROS default —
only what is written from the moment it matched. Both directions are
covered in-process by `tests/spatial_bridge_e2e.rs`, together with the
VOLATILE case that keeps the TRANSIENT_LOCAL one honest.

By hand, with the RMW built per "In a ROS-2 workspace" above and
`ros2.spatial_bridge.enabled: true` in
`/etc/zerodds/zerodds.d/50-spatial-bridge.yaml` (or `--config`/`--set` on the
node's command line):

```bash
# Terminal 1 — a native ROS 2 node subscribing to the bridged topic.
. ~/ros2_ws/install/setup.bash
RMW_IMPLEMENTATION=rmw_zerodds_cpp \
  ros2 topic echo /robot1/pose geometry_msgs/msg/PoseStamped

# Terminal 2 — a SpatialDDS FramedPose publish on the OTHER side of the
# bridge, via any zerodds-spatial-dds writer on the configured
# spatial_topic (sdds/default/FramedPose above) and domain; e.g. the
# crate-spatial-ros2 example, once it also writes onto the wire instead of
# only converting in-process.
#   -> terminal 1 must print the converted PoseStamped.

# The reverse direction: publish the ROS type natively and see it arrive as
# SpatialDDS. rmw_zerodds_cpp itself can publish; there is no ros2-native CLI
# for an arbitrary DCPS reader, so verify with a zerodds-dcps reader (e.g.
# zerodds-spy, or a small program against crates/spatial-ros2's
# BridgeFramedPose-equivalent type) subscribed to sdds/default/FramedPose:
RMW_IMPLEMENTATION=rmw_zerodds_cpp \
  ros2 topic pub /robot1/pose geometry_msgs/msg/PoseStamped \
  "{header: {frame_id: map}, pose: {position: {x: 1.0, y: 2.0, z: 3.0}}}"
#   -> the SpatialDDS-side reader must print a FramedPose with frame "map"
#      and position (1.0, 2.0, 3.0).
```

The SpatialDDS side's DDS type name follows the fixed convention
`spatial::<module>::<Type>` (e.g. `spatial::core::FramedPose`, mirroring the
Rust module path in `zerodds-spatial-dds`) — a SpatialDDS publisher/
subscriber this bridge talks to must use the same string; DDS matches by
topic name **and** type name, and this is not XTypes-hash-checked (same as
every hand-written `DdsType` outside the `idl-rust` codegen path).

`tf_message` (SpatialDDS `FrameTransform` bundled into `tf2_msgs/TFMessage`)
is refused by config name: `spatial_ros2::convert` has no reverse conversion
for it at all, and its forward one bundles a slice of transforms, not one
sample at a time, so it does not fit this bridge's per-sample pump. Use
`transform_stamped` for a 1:1 `FrameTransform` <-> `geometry_msgs/
TransformStamped` bridge instead.

## Tests

```bash
cargo test -p rmw-zerodds-shim
```

The Rust side. The C layer against a ROS 2 install runs through the
`rmw_c/run_*.sh` scripts (`ROS_PREFIX=<install>`): pytest over rclpy,
services, QoS translation, content filter, security, type adaptation,
dynamic type support (Jazzy only), the ament package.

## Status

- ✅ Crate skeleton, cdylib + staticlib + rlib
- ✅ cbindgen-generated `rmw_zerodds.h` (Phase-A API)
- ✅ ament_cmake wrapper files (`ament/CMakeLists.txt` + `package.xml`)
- ✅ pub-sub pipeline live (against the `zerodds-c-api` runtime)
- ✅ ros2 live smoke (Humble, via `rmw_c/run_ros2_pytest.sh`)
- ✅ service layer + wait-sets
- ✅ type hash REP-2009 (`rmw_zerodds_compute_type_hash`, RIHS SHA-256)
- ✅ REP-2007 type adaptation, rmw-layer half (`rmw_c/rmw_zerodds_ros2_ext.h`)
- ✅ REP-2008 accelerator loan memory, rmw-layer half (same header)
- 🟡 ROS 2 Jazzy dynamic type support (`rmw_zerodds_dynamic.cpp` +
  `src/dynamic.rs`) — interface table + unit tests done; e2e run on a
  real Jazzy host open, see "Dynamic types" below

## REP-2007 — type adaptation at the rmw layer

REP-2007 puts `rclcpp::TypeAdapter` in rclcpp and says why, then names what it
left to the middleware:

> Perhaps we can support a form of this feature in other languages in rcl or
> rmw in the future. One challenge in doing this is that it may require custom
> type support, which may be middleware specific.

`rmw_zerodds_ros2_ext.h` is that custom type support. `rmw_zerodds_type_adapter_t`
carries the adapter's (de)serialize half — the half REP-2007 calls "better for
inter-process communication" — and `rmw_zerodds_publish_adapted` /
`rmw_zerodds_take_adapted` run it directly against the wire buffer. The
`ros_message_type` is never materialized, so the copy REP-2007 describes as
avoidable ("An advantage of following the ROS 1 approach is that an extra copy
can be avoided") is not made. The same surface exists for the request and
response halves of a service; rcl composes actions from services plus topics, so
those are covered by the same two surfaces.

The endpoint is created from the `ros_message_type`'s rosidl type support and is
not touched by adaptation: DDS topic name, ROS type name and RIHS type hash stay
identical, so an adapted publisher matches a plain subscription and the bytes on
the wire are the same.

```bash
ROS_PREFIX=$CONDA_PREFIX rmw_c/run_type_adaptation_test.sh
```

## REP-2008 — hardware acceleration at the rmw layer

REP-2008 §"Kernel levels in ROS 2" places this package at two levels:

> Level 2 - ROS 2 core packages: […] we consider rclcpp, rcl, rmw, and the
> corresponding rmw_adapters for each supported communication middleware.
> Level 3 - ROS 2 underlayers: Groups together all accelerators below the ROS 2
> core layers […] including the communication middleware (e.g. DDS).

### Template for Vendors (REP-2008 §"Template for Vendors")

| Item | rmw_zerodds |
| --- | --- |
| Kernel levels served | 2 (rmw), 3 (the middleware itself) |
| Pillar I — ament extensions | consumed: `ament/CMakeLists.txt` honours `ROS_ACCELERATION` and `ROS_VITIS`, both `QUIET`-optional |
| Pillar II — firmware | not provided — firmware is the silicon vendor's `acceleration_firmware_<solution>` package |
| Acceleration technologies | technology-agnostic: the surface is host-memory provenance, not a kernel language |
| Accelerator-facing API | `rmw_zerodds_loan_memory_t` + `rmw_zerodds_publisher/subscription_get_loan_memory` |
| Loan alignment control | `rmw_zerodds_publisher_set_loan_alignment`, env `ZERODDS_LOAN_ALIGNMENT=page` |
| Cross-process buffers | `ZERODDS_DELIVERY_MODE=raw-same-host` (POSIX shared segment) or `iceoryx` |
| Reported level bitmask | `rmw_zerodds_acceleration_kernel_levels()` |

The concrete debt a DDS implementation owes an accelerator is memory
provenance: CUDA, ROCm and XRT can only DMA out of host memory they have pinned
or registered, and they pin whole pages. `rmw_zerodds_loan_memory_t` reports the
page-aligned extent covering a loaned message buffer — the exact arguments for
`cudaHostRegister`, `hipHostRegister` or `xrt::bo` — plus whether other
processes on the host can map the same bytes and under which filesystem link.
The application pins the loan once and the accelerator writes into the memory
DDS sends, which removes the staging copy REP-2008 §"Methodology" step 3.2 asks
to remove.

```bash
ROS_PREFIX=$CONDA_PREFIX rmw_c/run_accel_loan_memory_test.sh
```

## Dynamic types — ROS 2 Jazzy

ROS 2 Jazzy's runtime type reflection (`rmw/dynamic_message_type_support.h`,
`rosidl_dynamic_typesupport`) lets `rclcpp`/`rclpy` subscribe to a topic
without the message's compiled-in type support, reading a sample through
a `rosidl_dynamic_typesupport_dynamic_data_t` instead of a generated C++/
Python class. `rmw_cyclonedds_cpp` answers the three rmw entry points
this needs with `RMW_RET_UNSUPPORTED`; this shim answers them for real,
backed by ZeroDDS' own reflective `DynamicType`/`DynamicData`
(`crates/types/src/dynamic/`) rather than a second, parallel type system —
the same one `zerodds-dcps`'s `SerializedTypeSupport` uses for
runtime-described samples elsewhere in the repo.

* `rmw_serialization_support_init` returns a
  `rosidl_dynamic_typesupport_serialization_support_interface_t` — the
  192-function-pointer table `rosidl_dynamic_typesupport_fastrtps` fills
  for Fast DDS — implemented in
  `crates/rmw-zerodds-shim/rmw_c/rmw_zerodds_dynamic.cpp`, calling into
  `crates/rmw-zerodds-shim/src/dynamic.rs`'s ~90 `extern "C"` entry
  points.
* `rmw_take_dynamic_message[_with_info]` takes a sample into a
  `dynamic_data_t` over the same raw wire path the static introspection
  codec uses (`rmw_zerodds_take`), reading the CDR representation off the
  sample's own 4-byte encapsulation header rather than assuming one.
* Every dynamically built struct is `@final`, plain positional CDR — the
  same framing the static introspection codec in `rmw_zerodds.c` writes —
  measured byte-identical against a hand-computed plain-CDR encoding in
  `dynamic.rs`'s `a_mixed_alignment_struct_matches_plain_cdr_byte_for_byte`.
  An IDL-authored ROS 2 message declared `@appendable`/`@mutable` is
  outside that framing and is a named open point (see
  `docs/spec-coverage/ros2-rmw.md`'s "ROS 2 Jazzy Dynamic Type Support"
  section for the full list, including the two other named gaps: a
  member's `@default` literal is validated but not persisted, and
  `dynamic_data_serialize` cannot see a writer's negotiated
  `DataRepresentation` QoS).
* Guarded on `__has_include(<rosidl_dynamic_typesupport/types.h>)`, same
  as the rest of the Jazzy-only surface: Humble has neither the header
  nor the three rmw.h declarations, and the file (and
  `find_package(rosidl_dynamic_typesupport QUIET)` in
  `ament/CMakeLists.txt`) is a no-op there.

```bash
cargo test -p rmw-zerodds-shim --lib dynamic::
ROS_PREFIX=/opt/ros/jazzy rmw_c/run_dynamic_typesupport_test.sh
```
