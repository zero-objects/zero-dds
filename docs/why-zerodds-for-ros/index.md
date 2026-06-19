# Why ZeroDDS for ROS 2

*A pure-Rust, fully spec-compliant DDS that fixes the reasons people leave DDS — without leaving RTPS interop behind.*

---

## The short version

ROS 2 runs on DDS. DDS is a good standard with a hard reality: the default
configuration floods networks, breaks on WiFi, fails silently on QoS
mismatches, drops large messages, and needs expert XML tuning before it works.
Those are not rare edge cases — they are the **most-reported, most-recent**
problems in the ROS community, and they are the reason the project adopted an
alternate middleware (Zenoh) in 2023.

**ZeroDDS is a from-scratch, pure-Rust DDS implementation that stays on the
wire (native RTPS 2.5, interoperable with Fast DDS / Cyclone DDS / OpenDDS /
Connext) but removes the structural causes of those failures.** It speaks the
ROS 2 middleware ABI (`rmw_zerodds`), so it is a drop-in `RMW_IMPLEMENTATION`,
not a fork of ROS.

This trail does three things, per pain cluster:

1. **Describes the pain** — grounded in a fresh field scan of **349
   real reports** (GitHub issues, ROS Discourse, Stack Exchange, vendor blogs;
   see [`../ros2-dds-painpoints-research-2026-06.md`](../ros2-dds-painpoints-research-2026-06.md)).
2. **Cites the most recent ticket** as a concrete, checkable example.
3. **Explains how ZeroDDS removes it** — and how **you can reproduce the fix
   yourself** from the open harnesses.

> **For open-source validators:** every performance and interop claim below
> ships with the command that produced it. We *want* you to run them, break
> them, and file what you find. This page is a set of falsifiable claims, not a
> brochure.

---

## Why this matters more than it looks

ROS 2 is the de-facto standard for modern robotics R&D and a growing share of
production robotics. The pain is not theoretical — it is a daily tax measured
in lost lab afternoons, crashed demos, and "why don't my nodes talk" threads.
A scan of the field (2016–2026, newest dominating) breaks down like this:

| Pain cluster | Reports | Most-recent example |
|---|---|---|
| [Discovery](discovery.md) — multicast SDP, discovery storms, nodes not found | 62 | [Fast-DDS#6401](https://github.com/eProsima/Fast-DDS/issues/6401) (2026-05-18) |
| [Shared memory](shared-memory.md) — Iceoryx/SHM segfaults, `/dev/shm`, same-host fails | 52 | [rmw_cyclonedds#585](https://github.com/ros2/rmw_cyclonedds/issues/585) (2026-06-02) |
| [QoS silent no-match](qos-silent-fail.md) — incompatible QoS → no data, no error | 36 | [ros2#1562](https://github.com/ros2/ros2/issues/1562) (2024-05-10) |
| [Multicast / WiFi](multicast-wifi.md) — blocked, floods, dropouts | 34 | [turtlebot4#673](https://github.com/turtlebot/turtlebot4/issues/673) (2026-02-04) |
| [Cross-vendor / inter-distro interop](interop.md) | 32 | [rmw_cyclonedds#577](https://github.com/ros2/rmw_cyclonedds/issues/577) (2026-04-02) |
| [Large data / fragmentation](large-data.md) — images, point clouds, 262 kB ceiling | 29 | [Fast-DDS#5686](https://github.com/eProsima/Fast-DDS/issues/5686) (2025-03-05) |
| [DDS-Security / SROS2](security.md) | 22 | [Fast-DDS#5753](https://github.com/eProsima/Fast-DDS/issues/5753) (2025-04-08) |
| [Configuration complexity](config-complexity.md) — XML tuning, hidden prerequisites | 21 | [Discourse "I'm done tuning DDS"](https://discourse.openrobotics.org/t/im-done-manually-tuning-dds-parameters/54415) (2026-04-30) |
| [Docker / Kubernetes / cloud](docker-cloud.md) | 19 | [IsaacSim#407](https://github.com/isaac-sim/IsaacSim/issues/407) (2026-01-09) |
| [Performance / latency / CPU](performance.md) | 19 | [rmw_cyclonedds#559](https://github.com/ros2/rmw_cyclonedds/issues/559) (2026-03-03) |
| [Scaling / fleets / many nodes](scaling.md) | 16 | [autoware#6759](https://github.com/autowarefoundation/autoware/issues/6759) (2026-01-24) |
| [Migration to alternative middleware](migration.md) | 7 | [Alternative Middleware Report](https://discourse.openrobotics.org/t/ros-2-alternative-middleware-report/33771) (2023-09-27) |

Each row links to a page that follows the same shape: **the pain → the most
recent ticket → how ZeroDDS solves it → reproduce it yourself**.

---

## The standard: what ZeroDDS implements

ZeroDDS is not a DDS-flavoured transport. It is a complete, audited
implementation of the OMG DDS specification family — the same stack RTI,
eProsima, ZettaScale and OpenDDS implement, written in safe Rust.

| Specification | Scope | Status |
|---|---|---|
| **DDSI-RTPS 2.5** | Wire protocol (SPDP/SEDP, reliable, fragmentation, HB/ACKNACK) | Full — native interop with Fast DDS / Cyclone / OpenDDS / Connext |
| **DDS-DCPS 1.4** | Pub/Sub API, QoS, instances, listeners | Full |
| **DDS-XTypes 1.3** | TypeObject/TypeLookup, assignability, XCDR1 + XCDR2 | Full |
| **DDS-Security 1.2** | Authentication, access control, crypto, logging, tagging | Full — cross-vendor security matrix |
| **DDS-XML, DDS-XRCE, DDS-RPC** | XML profiles, Micro-DDS agent/client, services | Full |
| **Language PSMs** | C / C++ (PSM-Cxx) / Java / C# / Python / TypeScript | Full, codegen-driven |
| **ROS 2 RMW** | `rmw_zerodds` (REP-2003/2004/2005/2007/2008/2009) | Drop-in `RMW_IMPLEMENTATION`, live cross-RMW interop with `rmw_cyclonedds` |

RC1 is published: **97 crates on crates.io + docs.rs, 100 % documented.** It is
all open source.

---

## What we can do

- **Native RTPS 2.5 interop** — talks to Fast DDS, Cyclone DDS, OpenDDS and
  Connext on the wire. A ZeroDDS node and a `rmw_cyclonedds` node match and
  exchange data bidirectionally (verified 20/20 on `rt/chatter`).
- **Discovery without multicast** — unicast initial-peers (`ZERODDS_PEERS`,
  `ZERODDS_NO_MULTICAST`) give you working discovery on WiFi, in Docker, across
  subnets, with **no discovery server to deploy and babysit**.
- **Loud failures instead of silent ones** — an incompatible QoS match emits a
  `qos.incompatible` event with the exact offending policy, and a static
  `qos_check` CLI validates compatibility *before* you launch.
- **Large data that actually arrives** — application-level fragmentation with
  selective NACK_FRAG retransmit and a 16 MiB default reassembly cap (no silent
  drop at 1 MiB / 262 kB).
- **Variable-size zero-copy shared memory** — a length-prefixed SHM ring, not a
  fixed-size Iceoryx pool you have to dimension by hand.
- **Runs from MCU to server** — pure Rust `no_std + alloc`; the core builds for
  `thumbv7em-none-eabihf` (Cortex-M4F) at a ~1.6 MB footprint, and scales up to
  multi-robot fleets.
- **Memory-safe by construction** — safe Rust, `forbid(unsafe_code)` across the
  safe core; whole classes of the SHM segfaults and buffer races reported
  against C++ stacks are not expressible.

---

## How fast we are

All numbers are reproducible from the open examples and harnesses. Hardware and
method are stated so you can compare on your own boxes.

| Metric | Number | How to reproduce |
|---|---|---|
| Round-trip latency, loopback, 256 B | **p50 = 40 µs / p99 = 83 µs** (200 samples, 0 lost) | `latency_ping` / `latency_pong` |
| Round-trip latency, cross-machine over WiFi, 256 B | **p50 ≈ 4.3 ms** (full discovery, 0 lost) † | `latency_ping` / `latency_pong` across two hosts |
| Large-data throughput over WiFi (fragmented) | **10.8 MiB/s (~86 Mbit/s)** | `run_wifi_largedata.sh` |
| Large samples intact (2 / 4 / 8 MB) | byte-perfect, multicast-free | `run_largedata.sh` |
| All-to-all discovery, multicast-free | 50 participants in ~2.9 s, 100 in ~19.9 s | `ZERODDS_SCALE_N` scaling harness |
| Embedded footprint | ~1.6 MB, `thumbv7em-none-eabihf` | `cargo build --target thumbv7em-none-eabihf --no-default-features` |

† The cross-machine WiFi number required keeping the WiFi NIC awake; idle
802.11 power-save on the client otherwise drops the latency-sensitive unicast
discovery frames. This is an OS/AP power-management artifact (vendor-agnostic,
reproducible A/B with a packet capture), **not** a ZeroDDS limitation —
documented in
[`../interop/ros2-c3-large-data-wifi-followup.md`](../interop/ros2-c3-large-data-wifi-followup.md).

---

## Validate it yourself

This is the part that matters for an open-source audience. We do not ask you to
trust a benchmark slide — we ship the harnesses:

- **Cross-vendor, multicast-free discovery** vs Cyclone DDS:
  `crates/ros2-rmw/interop/run_multicast_free_xvendor.sh` — a ZeroDDS subscriber
  and a Cyclone talker discover each other with multicast fully disabled, and
  exchange 20/20 samples.
- **Live ROS 2 interop**: `crates/ros2-rmw/interop/run_interop.sh` — `rmw_zerodds`
  against a real `rmw_cyclonedds` talker/listener on `rt/chatter`.
- **Latency / throughput / large data**: the `latency_*`, `largedata_*` examples
  under `crates/dcps/examples/`.

If a claim on these pages does not reproduce on your hardware, that is a bug
report we want. The pain corpus
([`../ros2-dds-painpoints-research-2026-06.md`](../ros2-dds-painpoints-research-2026-06.md))
is also open — pick any ticket, reproduce it on your current RMW, then try it on
ZeroDDS.

---

## Honest status

ZeroDDS is at **1.0.0-rc.1**. The spec stack is complete and audited; the
cross-vendor interop, multicast-free discovery, large-data and QoS-loudness
claims are e2e-verified. Areas still being hardened and measured: head-to-head
latency/throughput comparison tables against each vendor, and broader
real-fleet scaling numbers. Where a claim is verified we say so; where it is
aspirational we mark it. The per-cluster pages are explicit about which is
which.

---

*Pages in this trail:*
[Discovery](discovery.md) ·
[Multicast / WiFi](multicast-wifi.md) ·
[QoS silent no-match](qos-silent-fail.md) ·
[Large data](large-data.md) ·
[Cross-vendor interop](interop.md) ·
[Shared memory](shared-memory.md) ·
[Configuration complexity](config-complexity.md) ·
[Scaling](scaling.md) ·
[Docker / cloud](docker-cloud.md) ·
[Security](security.md) ·
[Performance](performance.md) ·
[Migration](migration.md)
