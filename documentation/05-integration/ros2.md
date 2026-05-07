# ROS-2

`rmw-zerodds-shim` is a [ROS-2 RMW][rmw] (ROS Middleware) plugin
that makes ZeroDDS the underlying DDS for ROS-2 nodes.

## Supported distributions

| Distro | Status |
|---|---|
| Humble (LTS) | Supported |
| Iron | Supported |
| Jazzy | Supported |
| Rolling | Best-effort — RMW API is stable, but ament-cmake build glue tracks `ros2/ros2.repos` HEAD |

## Install

### From .deb (when published)

```bash
sudo apt install ros-humble-rmw-zerodds
```

### From source

```bash
cd ~/ros2_humble/src
git clone https://github.com/zero-objects/zero-dds.git
cd ~/ros2_humble
colcon build --packages-select rmw_zerodds
```

The native `librmw_zerodds.so` lives at
`/opt/ros/<distro>/lib/librmw_zerodds.so`; the package.xml +
CMakeLists.txt are in `crates/rmw-zerodds-shim/ament/`.

## Use it

Set the RMW implementation environment variable:

```bash
export RMW_IMPLEMENTATION=rmw_zerodds
ros2 run demo_nodes_cpp talker
# in another terminal, same env:
ros2 run demo_nodes_cpp listener
```

Verify:

```bash
ros2 node list
ros2 doctor --report
```

## QoS profiles

ROS-2 ships [REP-2003][rep-2003] / [REP-2009][rep-2009] standard
QoS profiles. They map onto DDS QoS automatically:

| ROS-2 profile | Maps to (DDS) |
|---|---|
| `default` | Reliable + Volatile + KeepLast(10) |
| `sensor_data` | BestEffort + Volatile + KeepLast(5) |
| `services_default` | Reliable + Volatile + KeepLast(10) |
| `parameters` | Reliable + Volatile + KeepLast(1000) |

The mapping is in `crates/ros2-rmw/src/rmw_qos_mapping.rs`.

## Type-system mapping

ROS-IDL types map onto DDS-XTypes per [REP-2008][rep-2008]. The
type-name conversion uses the `<package>::<namespace>::dds_::<Type>_`
convention (Spec §4.4):

| ROS-IDL | DDS type-name |
|---|---|
| `std_msgs/msg/String` | `std_msgs::msg::dds_::String_` |
| `geometry_msgs/msg/PoseStamped` | `geometry_msgs::msg::dds_::PoseStamped_` |

This ensures wire-compat with other RMWs (`rmw_cyclonedds_cpp`,
`rmw_fastrtps_cpp`).

## Topic-name mangling

Topics get a `rt/` prefix per REP-2003 §3, unless
`avoid_ros_namespace_conventions = true` is set on the QoS.

## RMW return codes

`rmw_zerodds` maps every internal `Result` onto an `rmw_ret_t`
per [REP-2007][rep-2007] §4. No silent error swallowing.

## Multi-node pub/sub demo

```bash
# Terminal 1
RMW_IMPLEMENTATION=rmw_zerodds ros2 run demo_nodes_cpp talker

# Terminal 2
RMW_IMPLEMENTATION=rmw_zerodds ros2 run demo_nodes_cpp listener

# Terminal 3 — observe the topic
RMW_IMPLEMENTATION=rmw_zerodds ros2 topic echo /chatter
```

## Cross-RMW interop

`rmw_zerodds` produces wire-compatible RTPS, so a `talker` running
on `rmw_cyclonedds_cpp` and a `listener` running on
`rmw_zerodds` interoperate without any bridge.

## Reading further

- `crates/ros2-rmw/README.md` — Rust-side mapping helpers.
- `crates/rmw-zerodds-shim/README.md` — extern-C wrapper.
- ROS-2 RMW interface — <https://design.ros2.org/articles/ros_middleware_interface.html>

[rmw]: https://design.ros2.org/articles/ros_middleware_interface.html
[rep-2003]: https://ros.org/reps/rep-2003.html
[rep-2007]: https://ros.org/reps/rep-2007.html
[rep-2008]: https://ros.org/reps/rep-2008.html
[rep-2009]: https://ros.org/reps/rep-2009.html
