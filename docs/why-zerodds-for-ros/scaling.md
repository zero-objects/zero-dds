# Scaling / Fleets / Many Nodes

← [Back to overview](index.md)

## The pain

ROS 2 creates many participants and many internal topics per node, so a fleet or
a large single robot multiplies DDS's discovery and matching cost (**16
reports**). Failures show up as:

- A Discovery Server becoming **unresponsive** past a few hundred participants.
- Memory blowing up when many readers/writers match, or when distros mix.
- Deadlocks when many readers and writers match under reliable TCP.
- Open questions in the community about how many participants an RMW even
  allows.

### Most recent example

**[autoware#6759 — "Fix [rmw_cyclonedds_cpp]: rmw_create_node: failed to create
domain, error"](https://github.com/autowarefoundation/autoware/issues/6759)**
(2026-01-24). A full self-driving stack hits domain/participant creation
failures — scaling limits surfacing in one of the largest real ROS 2
deployments.

### Reference list (most recent)

| Date | Source | Problem |
|---|---|---|
| 2026-01-24 | [autoware#6759](https://github.com/autowarefoundation/autoware/issues/6759) | Participant/domain creation fails in a large stack |
| 2025-09-09 | [ROS Discourse](https://discourse.openrobotics.org/t/how-many-dds-participants-are-currently-used-allowed-by-rmw/49976) | How many participants does an RMW even allow? |
| 2025-04-17 | [Fast-DDS#5767](https://github.com/eProsima/Fast-DDS/issues/5767) | Discovery Server unresponsive with many participants |
| 2025-01-15 | [rmw_fastrtps#797](https://github.com/ros2/rmw_fastrtps/issues/797) | Cross-distro sub/pub exhausts all memory |
| 2024-12-04 | [Fast-DDS#5235](https://github.com/eProsima/Fast-DDS/issues/5235) | Discovery Server deadlock with many matching endpoints |

## How ZeroDDS solves it

**No central server to overload, bounded peer state, and measured all-to-all
discovery.**

- **No Discovery Server bottleneck.** Multicast-free discovery is peer-to-peer
  unicast — there is no single server process to become unresponsive or deadlock
  at scale ([Fast-DDS#5767](https://github.com/eProsima/Fast-DDS/issues/5767),
  [#5235](https://github.com/eProsima/Fast-DDS/issues/5235)).
- **Bounded, explicit peer state.** `ZERODDS_MAX_PEER_PARTICIPANTS` caps how
  many participants are expanded per peer, so discovery state is bounded and
  predictable rather than open-ended.
- **Measured all-to-all discovery.** The scaling harness (`ZERODDS_SCALE_N`)
  brings up all-to-all, multicast-free meshes: ~50 participants in **~2.9 s**,
  100 in **~19.9 s**. These are honest current numbers on a single host — the
  point is the curve is measured and the mechanism (unicast, no server) has no
  central choke point.
- **Memory-safe matching.** The cross-distro "exhausts all memory" class
  ([rmw_fastrtps#797](https://github.com/ros2/rmw_fastrtps/issues/797)) comes
  from unbounded growth on malformed/mismatched discovery; ZeroDDS parses with
  explicit bounds and DoS caps.

## Why it no longer has to be a pain

Scaling pain concentrates at *the Discovery Server* and at *unbounded discovery
state*. ZeroDDS removes the central server (peer-to-peer unicast) and bounds peer
expansion explicitly, so adding robots adds linear, local unicast cost instead of
loading a shared choke point toward a cliff.

> **Honest status:** large-fleet (hundreds of real nodes) numbers are still
> being gathered. The single-host all-to-all curve above is verified; we want
> community runs on real fleets — see [Validate it yourself](index.md#validate-it-yourself).

## Reproduce it yourself

```bash
# All-to-all, multicast-free, N participants:
ZERODDS_SCALE_N=50 <scaling harness>     # ~2.9 s
ZERODDS_SCALE_N=100 <scaling harness>    # ~19.9 s
```

→ [Back to overview](index.md) · Next: [Docker / cloud](docker-cloud.md)
