# Configuration Complexity

← [Back to overview](index.md)

## The pain

Getting DDS to work well in ROS 2 routinely means becoming a part-time DDS
network engineer (**21 reports**): hundreds of XML knobs, per-vendor dialects
(Fast DDS profiles vs Cyclone XML vs Connext QoS), kernel tuning (`rmem_max`,
`ipfrag_*`), and hidden prerequisites that are nearly impossible to find. The
shipped defaults are not the robotics-/WiFi-appropriate ones, so "good enough"
takes days of trial and error.

- Localhost-only mode silently requires multicast enabled on the loopback
  interface (`ip link set lo multicast on`) — undocumented in practice.
- Selecting the right network interface, or making a Discovery Server
  introspectable, needs expert XML.
- A binary install can even go looking for a *paid* vendor by default.

### Most recent example

**[ROS Discourse — "I'm done manually tuning DDS parameters!"](https://discourse.openrobotics.org/t/im-done-manually-tuning-dds-parameters/54415)**
(2026-04-30). A long, well-received thread: hundreds of knobs, days of
trial-and-error, and still suboptimal results — a representative statement of the
configuration-complexity tax.

### Reference list (most recent)

| Date | Source | Problem |
|---|---|---|
| 2026-04-30 | [ROS Discourse](https://discourse.openrobotics.org/t/im-done-manually-tuning-dds-parameters/54415) | "Done tuning DDS": hundreds of knobs, days lost |
| 2025-12-09 | [ROS Discourse](https://discourse.openrobotics.org/t/dds-in-ros-2-consolidated-user-insights/51340) | OSRF "Consolidated User Insights" on DDS pain |
| 2025-08-15 | [ros2#1716](https://github.com/ros2/ros2/issues/1716) | Jazzy on Windows searches for *paid* RTI Connext |
| 2025-04-04 | [rmw_cyclonedds#537](https://github.com/ros2/rmw_cyclonedds/issues/537) | `failed to create domain, error Error` |
| 2025-04-02 | [cyclonedds#2201](https://github.com/eclipse-cyclonedds/cyclonedds/issues/2201) | Network-interface selection requires config spelunking |

## How ZeroDDS solves it

**Robotics-appropriate defaults out of the box, and environment variables
instead of XML dialects.**

- **`ros_defaults()` works out of the box.** A single `RuntimeConfig::ros_defaults()`
  sets the representation offers (XCDR1 + XCDR2) and the 16 MiB reassembly cap
  ROS actually needs — `rmw_zerodds` interops with a real ROS 2 talker
  **20/20 with no XML and no environment tuning**.
- **Configuration is environment variables, not an XML dialect.** Discovery
  (`ZERODDS_PEERS`, `ZERODDS_NO_MULTICAST`), interface pinning
  (`ZERODDS_INTERFACE`), sample caps (`ZERODDS_MAX_SAMPLE_BYTES`), peer limits
  (`ZERODDS_MAX_PEER_PARTICIPANTS`) — flat, documented knobs, not nested
  profile XML you debug with a parser.
- **Interface selection is one variable.** The "network-interface selection"
  pain ([cyclonedds#2201](https://github.com/eclipse-cyclonedds/cyclonedds/issues/2201))
  is `ZERODDS_INTERFACE=<ip>`, applied uniformly across UDP/TCP/SHM/UDS.
- **No hidden loopback prerequisite.** Unicast localhost discovery does not
  depend on multicast being enabled on `lo`.
- **No paid-vendor fallback.** The whole stack is open source (Apache-2.0 / MIT);
  there is no proprietary tier a default install can drift toward.

## Why it no longer has to be a pain

The configuration tax comes from *defaults tuned for data-center DDS, exposed
through per-vendor XML*. ZeroDDS ships defaults tuned for the robotics/WiFi case
and exposes the few knobs you actually need as flat environment variables — so
the median project needs zero configuration, and the rest needs a handful of
documented variables, not a weekend with a vendor's XML schema.

## Reproduce it yourself

```bash
# Out-of-the-box ROS interop with no XML / no env tuning:
crates/ros2-rmw/interop/run_interop.sh   # uses ros_defaults()
```

→ [Back to overview](index.md) · Next: [Scaling](scaling.md)
