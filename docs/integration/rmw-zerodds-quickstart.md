# rmw_zerodds — ROS 2 Quickstart

ZeroDDS als RMW-Implementation in ROS 2. Dieser Pfad nutzt den
`rmw-zerodds-shim`-Crate (Rust-cdylib) ueber `zerodds-c-api` und
`zerodds-ros2-rmw` (REP-2003/2004/2007-Mapping).

## Distro-Targets

| Distro | Ubuntu | ament_cmake | Status |
|--------|--------|-------------|--------|
| Humble | 22.04 LTS | 1.5.x | Phase-A (Pub-Sub) |
| Iron   | 22.04 LTS | 2.x   | Phase-A |
| Jazzy  | 24.04 LTS | 2.x   | Phase-A |

CI-Job: `ci/jobs/rmw-distro-build.yml` baut alle drei in
`ros:<distro>-ros-base`-Images und faehrt einen talker/listener-Smoke.

## Setup (auf einer ROS-2-Maschine)

```bash
# 1) ZeroDDS-Repo clonen + Rust-Toolchain installieren.
git clone https://gitlab.sandra-kessler.eu/fishermen21/zerodds ~/zerodds
cd ~/zerodds
cargo build -p rmw-zerodds-shim --release

# 2) Verifizieren dass die Library da ist.
ls target/release/librmw_zerodds.*
ls crates/rmw-zerodds-shim/include/rmw_zerodds.h

# 3) ament_cmake-Wrapper in einen ROS-2-Workspace.
mkdir -p ~/ros2_ws/src/rmw_zerodds
cp -r crates/rmw-zerodds-shim/ament/* ~/ros2_ws/src/rmw_zerodds/

# 4) colcon-Build (nutzt die Phase-1-Library).
cd ~/ros2_ws
. /opt/ros/humble/setup.bash
ZERODDS_HOME=$HOME/zerodds colcon build --packages-select rmw_zerodds
```

## Verwendung

```bash
. ~/ros2_ws/install/setup.bash
export RMW_IMPLEMENTATION=rmw_zerodds_cpp

# Terminal 1
ros2 topic pub /chatter std_msgs/msg/String "data: 'hello from zerodds'"

# Terminal 2
ros2 topic echo /chatter std_msgs/msg/String
```

## Was funktioniert (Phase-A)

- `ros2 topic pub` / `ros2 topic echo` — DCPS-Pub-Sub-Pipeline.
- `ros2 topic list` — SEDP-Discovery.
- `ros2 node list` — SPDP-Discovery.
- Multi-Domain (`ROS_DOMAIN_ID=42`).
- QoS-Profile-Mapping (REP-2003): default, sensor_data, parameters,
  services, parameter_events, system_default.

## Was Phase-B bringt

- Services (`ros2 service call`).
- Actions (`ros2 action send_goal`).
- Wait-Sets (rclcpp blockierende `spin_some`).
- Loaned-Messages (Zero-Copy ueber SHM).
- Type-Hash REP-2009 fuer schemaversion-stable Messaging.

Phase-A-Aufrufe der Phase-B-Funktionen liefern `RMW_RET_UNSUPPORTED`
und blockieren rclcpp's Plugin-Loader nicht.

## Troubleshooting

**`librmw_zerodds.so not found`** — `cargo build --release` lief
nicht oder `ZERODDS_HOME` zeigt nicht auf das Repo-Root.

**`rmw_init failed`** — Kein UDP-Socket konnte gebunden werden.
Domain-ID-Konflikt mit anderem RMW-Backend; `ROS_DOMAIN_ID=99`
versuchen.

**Topic discovery dauert > 30 s** — SPDP-Multicast vom Host
geblockt. `cyclonedds-tools`'s `ddsperf -i ${ROS_DOMAIN_ID}`
prueft Multicast-Reachability; ggf. `eth0` als
`CYCLONEDDS_URI`-Interface deklarieren.

**`ros2 service` haengt** — Phase-B-Feature, noch nicht
unterstuetzt. Bis dahin Service-Calls direkt via DDS-RPC
(`crates/rpc`) bypass'en.

## Spec-Compliance

| REP | Status | Datei |
|-----|--------|-------|
| REP-2003 (QoS-Profiles) | ✓ | `crates/ros2-rmw/src/qos_profiles.rs` |
| REP-2004 (Topic Naming) | ✓ | `crates/ros2-rmw/src/topic_mangling.rs` |
| REP-2005 (RMW interface) | partial | `crates/rmw-zerodds-shim/src/lib.rs` Phase-A |
| REP-2007 (RMW + ROS2 stack) | partial | dito |
| REP-2008 (Type Description) | partial | `crates/ros2-rmw/src/type_mapping.rs` |
| REP-2009 (Type Hash) | partial | `compute_type_hash` |

Vollstaendigkeitsmatrix: `docs/spec-coverage/rep-2003.md`,
`docs/spec-coverage/rep-2004.md` etc.
