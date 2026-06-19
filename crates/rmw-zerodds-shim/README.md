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
* **Services** — request/reply with a `[client_gid:16][seq:8]` correlation
  header (AddTwoInts round-trip green); `rmw_service_server_is_available`.
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
  `can_loan_messages` for fixed-POD), in two delivery modes selected by
  `ZERODDS_DELIVERY_MODE` (`docs/specs/zerodds-delivery-modes-1.0.md`):
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
# 1) Rust build in the ZeroDDS repo
cd ~/zerodds
cargo build -p rmw-zerodds-shim --release

# 2) ament_cmake wrapper in the ROS-2 workspace
mkdir -p ~/ros2_ws/src/rmw_zerodds
cp -r crates/rmw-zerodds-shim/ament/* ~/ros2_ws/src/rmw_zerodds/
cd ~/ros2_ws
ZERODDS_HOME=$HOME/zerodds \
  colcon build --packages-select rmw_zerodds
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

The CI job `ci/jobs/rmw-distro-build.yml` builds all three distros in
separate Docker layers.

## Symbols

The Rust crate exports `rmw_zerodds_*`-prefixed bridge functions (the
simplified DDS byte path). The C translation unit `rmw_c/rmw_zerodds.c` compiles
against the real Humble rmw headers and exports the genuine **unprefixed**
`rmw_*` ABI that stock rclpy/rclcpp load, bridging onto the prefixed functions.

All exported `rmw_*` entry points are implemented (none return
`RMW_RET_UNSUPPORTED`):

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

## Tests

```bash
cargo test -p rmw-zerodds-shim
```

Phase-A: 6 unit tests (codes, identifier, serialization format,
NULL tolerance, stubs).

## Status

- ✅ Crate skeleton, cdylib + staticlib + rlib
- ✅ cbindgen-generated `rmw_zerodds.h` (Phase-A API)
- ✅ ament_cmake wrapper files (`ament/CMakeLists.txt` + `package.xml`)
- ✅ pub-sub pipeline live (against the `zerodds-c-api` runtime)
- 🔲 ros2 live smoke (Humble, Iron, Jazzy) — Phase-A second stage
- 🔲 service layer + wait-sets — Phase-B
- 🔲 type hash REP-2009 — Phase-B
