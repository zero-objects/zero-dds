# rmw-zerodds-shim — ROS 2 RMW Implementation

Liefert `librmw_zerodds.{so,dylib,dll}` so dass rclcpp/rclpy ueber
`RMW_IMPLEMENTATION=rmw_zerodds_cpp` ZeroDDS als RMW-Backend nutzen
kann.

## Architektur

```
  rclcpp / rclpy / rclrs
      │  rmw API (REP-2007)
      ▼
  librmw_zerodds.{so,dylib,dll}    ◀── diese Crate
      │  ZeroDDS C-API (zerodds.h)
      ▼
  libzerodds.{so,dylib,dll}        (crates/zerodds-c-api)
      │  RTPS over UDP/TCP/SHM/UDS
      ▼
  Wire
```

REP-2007-Mapping (Topic-Mangling, QoS-Profile, Identifier-Constraints)
kommt aus `crates/ros2-rmw`. Wire-Pfad ist die ZeroDDS-DCPS-Runtime
ueber `zerodds-c-api`. Discovery laeuft via SPDP/SEDP automatisch.

## Phase-A vs Phase-B

**Phase-A (jetzt):** Pub-Sub-Pipeline + Initialisierung. Stubs fuer
Services, Wait-Sets, Loaning — diese returnen `RMW_RET_UNSUPPORTED`
und blockieren rclcpp's Plugin-Loader nicht.

**Phase-B (folgt):** Service-Layer (auf RPC-Crate), Wait-Sets,
Guard-Conditions, Loan-Borrowing, Type-Hash (REP-2009).

## Build

### Standalone (ohne ROS)

```bash
cargo build -p rmw-zerodds-shim --release
ls target/release/librmw_zerodds.*
ls crates/rmw-zerodds-shim/include/rmw_zerodds.h
```

### In einem ROS-2-Workspace

```bash
# 1) Rust-Build im ZeroDDS-Repo
cd ~/zerodds
cargo build -p rmw-zerodds-shim --release

# 2) ament_cmake-Wrapper im ROS-2-Workspace
mkdir -p ~/ros2_ws/src/rmw_zerodds
cp -r crates/rmw-zerodds-shim/ament/* ~/ros2_ws/src/rmw_zerodds/
cd ~/ros2_ws
ZERODDS_HOME=$HOME/zerodds \
  colcon build --packages-select rmw_zerodds
```

### Verwendung

```bash
. ~/ros2_ws/install/setup.bash
RMW_IMPLEMENTATION=rmw_zerodds_cpp \
  ros2 topic pub /chatter std_msgs/String "data: hello"

# In einem zweiten Terminal:
RMW_IMPLEMENTATION=rmw_zerodds_cpp \
  ros2 topic echo /chatter std_msgs/String
```

## Distro-Targets

* **ROS 2 Humble** — ament_cmake 1.5.x (Ubuntu 22.04 LTS)
* **ROS 2 Iron** — ament_cmake 2.x (Ubuntu 22.04 LTS)
* **ROS 2 Jazzy** — ament_cmake 2.x (Ubuntu 24.04 LTS)

CI-Job `ci/jobs/rmw-distro-build.yml` baut alle drei Distros in
separaten Docker-Layern.

## Symbole

Die Library exportiert `rmw_zerodds_*`-Praefixierte Funktionen.
rclcpp ruft sie via Plugin-Discovery durch. Phase-B addet die
unpraefixierten `rmw_*`-Aliase fuer rmw-side-by-side-Kompatibilitaet.

| Phase-A implementiert    | Phase-A stub (UNSUPPORTED) |
| ------------------------- | --------------------------- |
| `..._get_implementation_identifier` | `..._create_client`        |
| `..._get_serialization_format`      | `..._create_service`       |
| `..._init` / `_shutdown`            | `..._wait`                 |
| `..._create_node` / `_destroy_node` | `..._borrow_loaned_message`|
| `..._create_publisher` / `_publish` |                            |
| `..._create_subscription` / `_take` |                            |
| `..._buffer_free`                   |                            |

## Tests

```bash
cargo test -p rmw-zerodds-shim
```

Phase-A: 6 Unit-Tests (Codes, Identifier, Serialization-Format,
NULL-Tolerance, Stubs).

## Status

- ✅ Crate skeleton, cdylib + staticlib + rlib
- ✅ cbindgen-generierte `rmw_zerodds.h` (Phase-A-API)
- ✅ ament_cmake-Wrapper-Files (`ament/CMakeLists.txt` + `package.xml`)
- ✅ Pub-Sub-Pipeline live (gegen `zerodds-c-api`-Runtime)
- 🔲 ros2-Live-Smoke (Humble, Iron, Jazzy) — Phase-A zweite Etappe
- 🔲 Service-Layer + Wait-Sets — Phase-B
- 🔲 Type-Hash REP-2009 — Phase-B
