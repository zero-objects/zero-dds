# ROS 2 cross-RMW wire-interop harness

Counterpart to the CORBA `competitors/` harness. Proves that ZeroDDS speaks the
**ROS-2-over-DDS wire** and interoperates with the reference stack —
without a full ROS-2 installation, because ROS-2 topics **are DDS topics**:
`rmw_cyclonedds` is just CycloneDDS on `rt/<topic>` with the
ROS-2 IDL type + RMW default QoS. A CycloneDDS publisher that emits exactly
that is wire-identical to a real `ros2 topic pub`.

## Hosts
- **codepit** (Debian 13): CycloneDDS `/opt/cyclone` (libddsc 11.0.1, idlc,
  ddsperf), FastDDS `/opt/fastdds` 3.6.1. Loopback wire tests + pcap.
- **m1 (macOS, WiFi)**: for fragmentation/WiFi tests (pain cluster C3).

## Wire convention (rmw_dds_common)
- ROS topic `/chatter` → DDS topic **`rt/chatter`** (`rt/` prefix).
- ROS type `std_msgs/msg/String` → DDS type name **`std_msgs::msg::dds_::String_`**.
- RMW default QoS: RELIABLE + VOLATILE + KEEP_LAST(10).
- Encoding: XCDR1 (CDR), `std_msgs/String` is simple/final.

## Contents
- `std_msgs_string.idl` — the ROS-2 wire type (module-nested like rosidl).
- `cyclone_ros_talker.c` — CycloneDDS publisher on `rt/chatter` (= ROS-2 talker).
- `run_capture.sh` — builds the talker, captures an RTPS pcap (ground truth).

## Goal
1. Capture a ground-truth pcap of the ROS-2 wire (Cyclone).
2. The ZeroDDS `ros2-rmw` subscriber receives the Cyclone `rt/chatter` samples →
   wire-interop proof (analogous to cross-ORB JacORB/omniORB/TAO).
3. Reverse direction: ZeroDDS publisher → Cyclone subscriber.
