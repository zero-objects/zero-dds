# Docker / Kubernetes / Cloud

← [Back to overview](index.md)

## The pain

Containerized and cloud ROS 2 multiplies the discovery problem (**19 reports**):
container network namespaces, overlay networks, and Kubernetes CNIs do not pass
UDP multicast by default, so DDS discovery silently fails across pods/containers.

- Nodes in different containers cannot discover each other without `host`
  networking or a hand-built Discovery Server.
- A simulator/container can ignore the DDS config it was given and remain
  unreachable.
- When a host has both WiFi and Ethernet, a containerized node fails to register
  because the wrong interface is announced.
- Getting multicast through Kubernetes (Cilium and friends) is its own project.

### Most recent example

**[IsaacSim#407 — "Isaac Sim in Docker unreachable and ignores CycloneDDS
config"](https://github.com/isaac-sim/IsaacSim/issues/407)** (2026-01-09). A
containerized Isaac Sim instance is unreachable over ROS 2 and does not honour
the Cyclone DDS configuration it was handed — discovery-in-containers failing in
exactly the way the cluster predicts.

### Reference list (most recent)

| Date | Source | Problem |
|---|---|---|
| 2026-01-09 | [IsaacSim#407](https://github.com/isaac-sim/IsaacSim/issues/407) | Container unreachable, ignores DDS config |
| 2024-10-23 | [rmw_fastrtps#786](https://github.com/ros2/rmw_fastrtps/issues/786) | Docker host-net node fails to register with WiFi+Ethernet |
| 2024-03-27 | [ROS Discourse](https://discourse.openrobotics.org/t/ros-2-dds-flying-in-cloud-with-cilium-kubernetes/36845) | Making DDS multicast work under Cilium/Kubernetes |
| 2024-02-17 | [create3 discussion #549](https://github.com/iRobotEducation/create3_docs/discussions/549) | Discovery Server config needed for ROS 2 in Docker |
| 2024-02-14 | [ROS Discourse](https://discourse.openrobotics.org/t/ros-2-fast-dds-discovery-server-with-kubernetes/36086) | Discovery Server gymnastics for Kubernetes |

## How ZeroDDS solves it

**Unicast discovery + interface pinning is exactly the model containers and
clouds want.**

- **No multicast required, anywhere.** `ZERODDS_NO_MULTICAST=1` + `ZERODDS_PEERS`
  is unicast end-to-end, which is precisely what overlay networks and Kubernetes
  CNIs *do* pass. You address pods/containers by IP/service — no multicast for
  the CNI to drop, no Discovery Server pod to operate.
- **Interface pinning fixes the multi-interface registration failure.**
  `ZERODDS_INTERFACE=<ip>` binds and announces one interface across all
  transports, so the "WiFi+Ethernet host in Docker fails to register"
  ([rmw_fastrtps#786](https://github.com/ros2/rmw_fastrtps/issues/786)) failure
  is a one-variable fix.
- **Config is honoured, not ambient.** Discovery configuration is explicit
  environment variables read at startup — there is no separate XML the runtime
  can silently ignore.
- **TCP where overlays prefer it.** A first-class TCP transport is available for
  networks that only forward TCP cleanly.

## Why it no longer has to be a pain

Container/cloud pain is *multicast through networks that don't forward
multicast*. ZeroDDS's default-capable unicast discovery plus interface pinning
maps directly onto how container and cloud networking actually routes traffic —
the deployment becomes "list the peer IPs", not "operate a Discovery Server and
fight the CNI."

## Reproduce it yourself

```bash
# Unicast, multicast-free discovery (the container/cloud model), cross-vendor:
crates/ros2-rmw/interop/run_multicast_free_xvendor.sh
```

→ [Back to overview](index.md) · Next: [Security](security.md)
