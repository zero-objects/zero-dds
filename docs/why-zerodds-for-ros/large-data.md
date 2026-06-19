# Large Data / Fragmentation

← [Back to overview](index.md)

## The pain

Robotics moves big payloads — camera frames, point clouds, maps, occupancy
grids. UDP datagrams are ~64 kB max, so DDS must fragment large samples and
reassemble them reliably. In practice this is a recurring failure surface
(**29 reports**):

- Messages above an internal threshold are **silently dropped** — the
  notorious ~262 kB ceiling in some Fast DDS configs — so a point cloud just
  never arrives, with no error.
- On lossy links (WiFi), a single lost fragment stalls reassembly: the kernel
  IP-fragmentation buffer fills, and on some stacks the receiver stops accepting
  data for seconds (a vendor-agnostic, kernel-level failure).
- Large messages spike latency and bandwidth unpredictably, and block unrelated
  callbacks while the big sample is in flight.

### Most recent example

**[Fast-DDS#5686 — "FastDDS High Latency using Large Data"](https://github.com/eProsima/Fast-DDS/issues/5686)**
(2025-03-05). Enabling the large-data path produces high, inconsistent latency —
the large-message transport behaves very differently from the small-message
path, which is exactly the kind of surprise that makes "just send the image"
unreliable.

### Reference list (most recent)

| Date | Source | Problem |
|---|---|---|
| 2025-03-05 | [Fast-DDS#5686](https://github.com/eProsima/Fast-DDS/issues/5686) | High, inconsistent latency on the large-data path |
| 2024-11-15 | [cyclonedds#2139](https://github.com/eclipse-cyclonedds/cyclonedds/issues/2139) | "Unusual performance" with large messages |
| 2024-04-19 | [ros2#1544](https://github.com/ros2/ros2/issues/1544) | Inconsistent bandwidth on image transmission |
| 2024-04-14 | [Fast-DDS#4684](https://github.com/eProsima/Fast-DDS/issues/4684) | Send-buffer sizing breaks past `net.core.wmem_max` |
| 2024-03-12 | [ROS Discourse](https://discourse.openrobotics.org/t/ros-2-and-large-data-transfer-on-lossy-networks/36598) | Large data on lossy networks: reassembly stalls |

## How ZeroDDS solves it

**No silent cap, selective retransmit, and a transport that doesn't tank on a
single lost fragment.**

- **No silent drop.** ZeroDDS's reassembly cap is **16 MiB by default**
  (configurable via `ZERODDS_MAX_SAMPLE_BYTES`). The old "samples over N bytes
  vanish" failure was a 1 MiB Phase-1 cap that we found and removed; 2 / 4 / 8 MB
  samples reassemble byte-perfect through the full DCPS stack.
- **Selective fragment retransmit.** ZeroDDS implements DATA_FRAG / NACK_FRAG
  with a fragment assembler that has DoS caps. A lost fragment triggers a
  NACK_FRAG that re-requests *only the missing fragments*, not the whole sample
  — verified byte-identical at 30 % packet loss. The reassembly buffer is the
  application's, with explicit caps, so the kernel-IP-fragmentation stall does
  not apply.
- **WiFi-safe fragment size.** Application-level fragmentation at a WiFi-safe MTU
  keeps each fragment inside a single link-layer frame, so the lossy-network
  reassembly cliff is avoided by construction.
- **Variable-size zero-copy for same-host.** For the same-machine path, ZeroDDS
  has a length-prefixed shared-memory ring (see [shared memory](shared-memory.md))
  — variable-size, so point clouds and images do not need a hand-dimensioned
  fixed pool.

## Why it no longer has to be a pain

The large-data cluster is *silent caps* + *all-or-nothing reassembly* + *a
large-data path that behaves nothing like the small-data path*. ZeroDDS removes
the silent cap, retransmits at fragment granularity, and keeps fragmentation on
one well-tested path — so "just send the 4 MB point cloud" is the default that
works, including over lossy WiFi.

## Reproduce it yourself

```bash
# 2/4/8 MB samples through the full DCPS stack, byte-perfect, multicast-free:
crates/ros2-rmw/interop/run_largedata.sh

# Same, over a real WiFi link (throughput ~10.8 MiB/s):
crates/ros2-rmw/interop/run_wifi_largedata.sh
```

→ [Back to overview](index.md) · Next: [Cross-vendor interop](interop.md)
