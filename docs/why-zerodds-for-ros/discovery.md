# Discovery

← [Back to overview](index.md)

## The pain

DDS discovery is the single most-reported source of "my ROS 2 nodes don't talk"
(**62 reports** in the field scan). Three structural problems recur:

1. **Multicast-based Simple Discovery (SDP) is fragile and noisy.** It depends
   on UDP multicast, which is dropped or rate-limited on WiFi, in Docker, and on
   managed corporate/academic networks. Where it works, its traffic grows with
   the number of endpoints — and ROS 2 creates many internal topics per node, so
   discovery traffic can *drown out the actual data* at fleet scale.
2. **The Discovery Server "fix" is itself fragile.** Restarting the server, or a
   node, frequently leaves endpoints permanently unmatched until everything is
   restarted in the right order; it needs expert XML and CLI `SUPER_CLIENT`
   configuration to even introspect.
3. **Defaults discover too much.** Unrelated robots on the same network discover
   each other and can trigger unintended motion.

### Most recent example

**[Fast-DDS#6401 — "Unexpected piggyback HB to all matched readers breaks EDP
recovery loop after sleep/wake cycle"](https://github.com/eProsima/Fast-DDS/issues/6401)**
(2026-05-18). After a sleep/wake cycle in a three-node Simple Discovery
topology, one pair of nodes permanently fails to re-match because an async
piggyback Heartbeat is broadcast to *all* matched readers, corrupting a third
node's re-match state machine.

### Reference list (most recent)

| Date | Source | Problem |
|---|---|---|
| 2026-05-18 | [Fast-DDS#6401](https://github.com/eProsima/Fast-DDS/issues/6401) | EDP recovery breaks after sleep/wake (piggyback HB) |
| 2026-05-11 | [Fast-DDS#6346](https://github.com/eProsima/Fast-DDS/issues/6346) | Remote reader/writer no longer discovered in 3.5.0+ |
| 2025-10-14 | [Fast-DDS#5872](https://github.com/eProsima/Fast-DDS/issues/5872) | DataReader gets no data after Discovery Server restart |
| 2025-06-23 | [rmw_cyclonedds#541](https://github.com/ros2/rmw_cyclonedds/issues/541) | Listener gets no message with one RMW but fine with another |
| 2022-10-05 | [ROS Discourse](https://discourse.openrobotics.org/t/proposed-changes-to-how-ros-performs-discovery-of-nodes/27640) | OSRF: defaults discover too much *and* flood the network |
| 2020-11-17 | [ROS Discourse](https://discourse.openrobotics.org/t/new-discovery-server/17383) | SDP traffic 93 % higher; drowns out data at 50–200 nodes |

## How ZeroDDS solves it

**Discovery is direct unicast peer addressing — no multicast, no server.**

- **Multicast-free unicast discovery.** Set `ZERODDS_PEERS` to the peer IPs (or
  `ip:port`) and `ZERODDS_NO_MULTICAST=1`. ZeroDDS sends SPDP to each peer's
  well-known RTPS port (`7400 + 250·domain + 10 + 2·pid`). No multicast packet
  ever leaves the host, so WiFi/Docker/subnet multicast handling is irrelevant.
- **No Discovery Server to restart.** Because peers address each other
  directly, there is no separate server process whose restart leaves the mesh in
  a half-matched state — the entire class of "DataReader gets no data after
  server restart" ([Fast-DDS#5872](https://github.com/eProsima/Fast-DDS/issues/5872))
  does not exist.
- **Deterministic re-match.** ZeroDDS's SEDP re-announces and re-matches on a
  defined schedule; the "permanently unmatched after sleep/wake" failure mode is
  driven by direct, idempotent peer state, not a fragile piggyback-HB side
  effect.
- **The "listener gets no message" class is a known, fixed bug for us.**
  [rmw_cyclonedds#541](https://github.com/ros2/rmw_cyclonedds/issues/541) is the
  keyed-vs-keyless entity-kind mismatch family — a keyless type announced with a
  WithKey entity id is silently rejected by the peer's topic-kind match. ZeroDDS
  consults `DdsType::HAS_KEY` to emit the correct entity kind, which is exactly
  what made ZeroDDS ↔ `rmw_cyclonedds` interop go 20/20 bidirectional.
- **Scope is opt-in, not accidental.** Peers are an explicit list, so unrelated
  robots on the same network do not discover each other by default.

## Why it no longer has to be a pain

The root cause of the discovery cluster is *indirection and broadcast*: multicast
you don't control, plus a server (or piggyback side effects) whose state can
desync. ZeroDDS replaces both with **explicit, direct, unicast peer addressing**
— the same thing teams end up hand-building with Discovery Servers and XML, but
as a first-class, out-of-the-box mode with no extra process.

## Reproduce it yourself

```bash
# Multicast-free discovery across vendors (ZeroDDS sub ↔ Cyclone talker,
# multicast fully disabled on both): expect matched=1, 20/20 samples.
crates/ros2-rmw/interop/run_multicast_free_xvendor.sh

# rmw_zerodds against a real rmw_cyclonedds talker/listener on rt/chatter.
crates/ros2-rmw/interop/run_interop.sh
```

→ [Back to overview](index.md) · Next: [Multicast / WiFi](multicast-wifi.md)
