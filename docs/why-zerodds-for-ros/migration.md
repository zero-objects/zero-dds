# Migration / Alternative Middleware

← [Back to overview](index.md)

## The pain

The clearest signal that DDS pain is real: in 2023 the ROS project officially
adopted an **alternative middleware** (Zenoh, `rmw_zenoh`) because DDS, as the
sole middleware, did not work out of the box for a large part of the community
(**7 reports**, plus this is the conclusion the rest of the corpus points to).

- The official Alternative Middleware Report cites network-wide crashes from DDS
  multicast packet storms, DDS not working out of the box on managed networks,
  and the need for expert, application-specific DDS configuration.
- Zenoh was selected as the most-recommended alternative.

### Most recent / flagship example

**[ROS 2 Alternative middleware report](https://discourse.openrobotics.org/t/ros-2-alternative-middleware-report/33771)**
(2023-09-27, OSRF). The canonical statement of why DDS alone was deemed
insufficient, and the decision to add a non-DDS middleware.

### Reference list

| Date | Source | Point |
|---|---|---|
| 2023-09-27 | [ROS 2 Alternative middleware report](https://discourse.openrobotics.org/t/ros-2-alternative-middleware-report/33771) | Official: DDS insufficient alone → adopt Zenoh |
| 2023-10-30 | [Eclipse newsroom](https://newsroom.eclipse.org/eclipse-newsletter/2023/october/eclipse-zenoh-selected-alternate-ros-2-middleware) | Zenoh selected as alternate ROS 2 middleware |
| 2024-06-12 | [ZettaScale news](https://www.zettascale.tech/news/zenoh-experimental-support-lands-in-ros-2/) | Zenoh experimental support lands in ROS 2 |
| 2024-07-03 | [arXiv 2407.03091](https://arxiv.org/abs/2407.03091) | Middleware comparison for multi-robot mesh networks |
| 2025-01-03 | [ROS Discourse](https://discourse.openrobotics.org/t/rmw-zenoh-binaries-for-rolling-jazzy-and-humble/41395) | rmw_zenoh binaries shipped for Rolling/Jazzy/Humble |

## The ZeroDDS position

**You don't have to leave DDS to escape DDS pain.**

Zenoh fixes the ergonomics (router-/broker-style discovery, works on WiFi and in
the cloud) by leaving the RTPS wire — which means a Zenoh fleet no longer speaks
native DDS, and bridging back to existing DDS systems is a separate component.
For the large installed base of DDS robots, sensors, and tooling, that is a real
cost.

**ZeroDDS takes the other path: fix the reasons people left DDS, while staying on
the RTPS wire.**

- **Discovery ergonomics like Zenoh, without leaving RTPS.** Multicast-free
  unicast peers (no broadcast storms, works on WiFi/Docker/cloud) — but the wire
  is still native RTPS 2.5, so ZeroDDS nodes interoperate directly with the
  existing Fast DDS / Cyclone / OpenDDS / Connext fleet (verified 20/20 with real
  `rmw_cyclonedds`).
- **The structural fixes, not a workaround.** Loud QoS failures, no silent
  large-data cap, variable-size zero-copy SHM, robotics-appropriate defaults,
  full DDS-Security — the specific clusters this trail documents.
- **Standard-preserving.** A complete, audited OMG DDS spec stack means existing
  DDS tooling, type systems and security models keep working; there is no new
  protocol to bridge.
- **Memory-safe, MCU-to-server.** Pure Rust, `forbid(unsafe_code)` safe core,
  `no_std + alloc` for embedded — a property neither the incumbent C++ stacks nor
  a separate-protocol middleware offers.

## Why this is the better migration

The choice the community was forced into was "keep DDS and keep the pain" *or*
"adopt Zenoh and lose native DDS interop." ZeroDDS is the third option: **keep
the DDS standard and the wire interop, and lose the pain** — so a team can drop
in `rmw_zerodds` on one robot, interoperate with the rest of its DDS fleet
unchanged, and validate the improvement incrementally.

## Validate it yourself

This whole trail is a set of falsifiable, reproducible claims. Start anywhere:

```bash
crates/ros2-rmw/interop/run_interop.sh                 # live ROS 2 interop
crates/ros2-rmw/interop/run_multicast_free_xvendor.sh  # cross-vendor, no multicast
crates/ros2-rmw/interop/run_largedata.sh               # large data, byte-perfect
```

→ [Back to overview](index.md)
