# Multicast / WiFi

← [Back to overview](index.md)

## The pain

DDS discovery defaults to UDP multicast, and the data path leans on broadcast
assumptions that wireless networks do not honour (**34 reports**). On WiFi:

- Multicast is rate-limited or dropped by access points, so discovery silently
  fails — the canonical "works on my desk, dies in the lab" failure.
- Where multicast *is* allowed, discovery traffic over WiFi can fragment and
  behave like a self-inflicted mini-DDoS, causing second-long dropouts that
  crash multi-robot setups.
- Unconfigured multi-interface hosts announce *every* interface, then stream
  point clouds and LiDAR to addresses that route off-network and saturate
  uplinks for days.

The community's own conclusion: working out of the box on ordinary WiFi is "the
minimum viable product" for ROS 2 — and stock DDS fails that bar.

### Most recent example

**[turtlebot4#673 — "Configuring Fast DDS Discovery Server to use TCP to bypass
firewall UDP flood protection"](https://github.com/turtlebot/turtlebot4/issues/673)**
(2026-02-04). To get a TurtleBot 4 working on a managed wireless network, users
have to stand up a Discovery Server *and* switch the transport to TCP, purely to
get around the network's UDP-flood protection tripping on DDS traffic.

### Reference list (most recent)

| Date | Source | Problem |
|---|---|---|
| 2026-02-04 | [turtlebot4#673](https://github.com/turtlebot/turtlebot4/issues/673) | Need Discovery Server + TCP to survive WiFi UDP-flood protection |
| 2025-11-05 | [Cyclone WiFi gist](https://gist.github.com/robosam2003/d5fcfaf4bfd55298d86c1460cb7fc60c) | Hand-tuned XML to make Cyclone work on enterprise WiFi+Ethernet |
| 2025-08-15 | [arXiv 2508.11366](https://arxiv.org/html/2508.11366v1) | Whole paper on optimizing ROS 2 comms for wireless |
| 2025-02-10 | [eProsima "ROS 2 Easy Mode"](https://www.eprosima.com/news/forget-packet-loss-forget-discovery-hassles-meet-ros-2-easymode) | Vendor ships an "easy mode" to hide discovery/packet-loss pain |
| 2022-11-25 | [ROS Discourse](https://discourse.openrobotics.org/t/ros2-wifi-multicast-multi-robot-and-igmp-snooping/28516) | Multicast over WiFi → 1 s dropouts → drone crashes |
| 2022-05-24 | [ROS Discourse](https://discourse.openrobotics.org/t/unconfigured-dds-considered-harmful-to-networks/25689) | Unconfigured DDS floods networks for days |

## How ZeroDDS solves it

**Remove the multicast dependency entirely, and announce only the interface you
mean.**

- **Zero multicast on the wire.** `ZERODDS_NO_MULTICAST=1` + `ZERODDS_PEERS`
  gives full discovery over plain unicast UDP. There is nothing for the AP's
  IGMP snooping, multicast rate-limiting, or UDP-flood protection to trip on.
- **TCP transport is native, not a workaround.** Where a network only passes
  TCP, ZeroDDS has a first-class TCP transport — you select it, you don't bolt a
  Discovery Server on to get there.
- **Interface pinning for multi-homed hosts.** `ZERODDS_INTERFACE=<ip>` binds
  send/receive and announces exactly one interface across all transports
  (UDP/TCP/SHM/UDS), so a host with a real NIC plus Docker/VM virtual
  interfaces never announces or streams to addresses that route off-network —
  the "unconfigured DDS considered harmful" failure cannot happen.
- **Honest about the one remaining WiFi gotcha.** Idle 802.11 power-save on a
  WiFi *client* can drop latency-sensitive unicast discovery frames until the
  NIC is woken. We root-caused this with a clean A/B packet capture; it is an
  OS/AP power-management artifact that affects every DDS vendor identically, and
  the mitigation lives at the OS/AP layer, not in the stack. See
  [`../interop/ros2-c3-large-data-wifi-followup.md`](../interop/ros2-c3-large-data-wifi-followup.md).

## Why it no longer has to be a pain

Every WiFi failure in the corpus traces back to *depending on multicast/broadcast
behaviour the wireless layer does not guarantee*, plus *announcing interfaces you
didn't mean to*. ZeroDDS's default-capable unicast discovery plus interface
pinning removes both — the same outcome teams reach after days of XML tuning and
Discovery-Server deployment, available as a two-environment-variable setup.

## Reproduce it yourself

```bash
# Large data over a real WiFi link, multicast-free, byte-perfect:
crates/ros2-rmw/interop/run_wifi_largedata.sh

# Multicast-free cross-vendor discovery (no multicast packet emitted at all):
crates/ros2-rmw/interop/run_multicast_free_xvendor.sh
```

→ [Back to overview](index.md) · Next: [QoS silent no-match](qos-silent-fail.md)
