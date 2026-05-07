# 03 – Configuration

Every knob ZeroDDS exposes, organised by what you tune and when.

## Sub-stations

- [Runtime config](runtime-config.md) — `RuntimeConfig` fields,
  domain ID, tick period, multicast group, lease duration.
- [QoS policies](qos-policies.md) — Reliability, Durability,
  History, Deadline, Liveliness, Lifespan, Ownership, Partition,
  Resource-Limits, full reference per policy.
- [Transport](transport.md) — UDP / TCP / SHM / UDS / TSN —
  pick the right transport for your topology.
- [Security](security.md) — Governance + Permissions XML, plugin
  config, certificate management.
- [Observability](observability.md) — Sink injection, atomic
  stats, OTel bridge.

## Configuration matrix

When you change one knob you often have to think about another.
This is the cheat-sheet:

| Goal | Knobs to set |
|---|---|
| Lossless reliable delivery | `Reliability=Reliable`, `History=KeepAll`, `ResourceLimits.max_samples` high |
| Late-joining replay | `Durability=TransientLocal`, `History=KeepLast(N)` |
| Hard real-time | `tick_period=5ms`, `RealtimeFifo{50}` via `zerodds-rt-linux`, `History=KeepLast(1)`, `BestEffort` |
| Cross-vendor interop | RTPS 2.1 vendor handling enabled (default) |
| Audit trail | `RuntimeConfig.observability = StderrJsonSink::new()` |
| Production crypto | enable `security` feature, configure governance + permissions XML, deploy CA chain |

## Defaults

ZeroDDS defaults match the OMG DDS 1.4 spec defaults wherever there
is one. Where the spec defers to vendor choice, ZeroDDS picks the
choice that "just works" for hello-world (e.g.
`tick_period = 50ms`, `spdp_period = 5s`, KeepLast depth = 1).

## Source of truth

This documentation describes what the knobs do; the actual
defaults live in `RuntimeConfig::default()` and the various
`*QosPolicy::default()` impls. When in doubt, read the rustdoc.
