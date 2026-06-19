# Cross-Vendor / Inter-Distro Interop

← [Back to overview](index.md)

## The pain

DDS *promises* interoperability — that is the whole point of a wire standard.
In ROS 2 practice it frequently breaks (**32 reports**):

- **Mixed-RMW fleets don't talk.** A `rmw_fastrtps` node and a `rmw_cyclonedds`
  node on the same topic may not exchange data; services and actions (built on
  pub/sub) are effectively vendor-locked even when plain pub/sub works.
- **Cross-vendor deserialization mismatches** can be worse than a no-match — a
  malformed cross-RMW request has triggered out-of-memory on the server.
- **Inter-distro is unsupported.** A Humble node and an Eloquent/Jazzy node on
  the same domain often cannot communicate, stranding incremental fleet upgrades.
- **XTypes encoding drift.** Even compliant-looking stacks disagree on CDR /
  XCDR2 encoding details, so type matching silently fails.

### Most recent example

**[rmw_cyclonedds#577 — "Cross-RMW service interoperability: ListParameters
request from rmw_cyclonedds_cpp client can be misdeserialized and trigger OOM on
rmw_fastrtps_cpp server"](https://github.com/ros2/rmw_cyclonedds/issues/577)**
(2026-04-02). A cross-vendor service call is not just incompatible — it is
*mis-deserialized* into an allocation that crashes the server. Interop failure
as a denial-of-service.

### Reference list (most recent)

| Date | Source | Problem |
|---|---|---|
| 2026-04-02 | [rmw_cyclonedds#577](https://github.com/ros2/rmw_cyclonedds/issues/577) | Cross-RMW service deserialization → OOM crash |
| 2025-06-12 | [RTI KB](https://community.rti.com/kb/xtypes-compliance-mismatch) | Connext default CDR non-compliant with XTypes 1.3 |
| 2025-05-14 | [ROS Discourse](https://discourse.openrobotics.org/t/incompatability-between-distributions/43747) | Incompatibility between ROS 2 distributions |
| 2024-09-18 | [ROS Discourse](https://discourse.openrobotics.org/t/difference-between-dds-design-and-reality/39669) | "DDS design vs reality": services/actions vendor-locked |
| 2024-08-05 | [cyclonedds#2062](https://github.com/eclipse-cyclonedds/cyclonedds/issues/2062) | Cyclone ↔ Micro XRCE-DDS comms |

## How ZeroDDS solves it

**Interop is the design center, and it is continuously tested against four
vendors.**

- **Native RTPS 2.5 on the wire.** ZeroDDS is verified interoperable with
  Cyclone DDS, Fast DDS, OpenDDS and RTI Connext, maintained as a cross-vendor
  matrix (including a security matrix) — the same places where Fast DDS ↔
  Cyclone breaks in the field are regression cells for us.
- **Live ROS 2 interop, both directions.** `rmw_zerodds` exchanges data with a
  real `rmw_cyclonedds` talker/listener on `rt/chatter` **20/20 in both
  directions**. The bug that originally blocked this (keyed-vs-keyless entity
  kind) is fixed by consulting `DdsType::HAS_KEY`.
- **XCDR1 *and* XCDR2.** ZeroDDS models `DataRepresentationQosPolicy` and offers
  both encodings; `ros_defaults()` offers XCDR1 for ROS writers out of the box,
  so the "compliant-but-doesn't-match" encoding drift is handled. ZeroDDS's
  XCDR2 alignment was validated byte-for-byte against a cross-vendor capture.
- **Full XTypes 1.3 + DDS-RPC.** TypeObject/TypeLookup and assignability are
  implemented, and the DDS-RPC spec (services) is implemented to the standard —
  the foundation services/actions need to stop being vendor-locked.
- **Memory-safe parsing.** A malformed cross-vendor request cannot be
  mis-deserialized into an OOM the way [rmw_cyclonedds#577](https://github.com/ros2/rmw_cyclonedds/issues/577)
  describes: decoding runs in safe Rust with explicit bounds and DoS caps.

## Why it no longer has to be a pain

Interop breaks when each vendor's "compliant" diverges in the encoding and
entity-kind details, and when parsers trust the wire. ZeroDDS treats
cross-vendor interop as a first-class, continuously-tested requirement (four
vendors, both directions) and parses defensively — so a heterogeneous fleet is a
supported configuration, not a gamble.

## Reproduce it yourself

```bash
# rmw_zerodds ↔ real rmw_cyclonedds, bidirectional, on rt/chatter:
crates/ros2-rmw/interop/run_interop.sh

# Cross-vendor, multicast-free:
crates/ros2-rmw/interop/run_multicast_free_xvendor.sh
```

See also the cross-vendor validation record in
[`../spec-coverage/cross-vendor-validation.md`](../spec-coverage/cross-vendor-validation.md).

→ [Back to overview](index.md) · Next: [Configuration complexity](config-complexity.md)
