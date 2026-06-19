# QoS Silent No-Match

← [Back to overview](index.md)

## The pain

DDS matches a publisher and subscriber only if their QoS is compatible
(reliability, durability, history, deadline, liveliness…). When it is *not*
compatible, the spec behaviour is to **silently not match** — no data, and often
no error the application ever sees (**36 reports**). The result is the most
demoralizing ROS 2 debugging session there is: everything looks connected,
`ros2 topic list` shows the topic, and not a single message arrives.

- A sensor driver publishes BEST_EFFORT; your node subscribes RELIABLE → no
  match, no message, no log.
- `transient_local` (latched) on one side only → silent no-match.
- The community position is that QoS compatibility is "too strict" and fails in
  a way that is invisible to non-experts.

### Most recent example

**[ros2#1562 — "QoS compatibility is too strict, should be more user-friendly and
flexible"](https://github.com/ros2/ros2/issues/1562)** (2024-05-10). A
maintainer-level request acknowledging that the current QoS-compatibility model
produces silent failures that are hostile to users, and asking for friendlier,
more visible behaviour.

### Reference list (most recent)

| Date | Source | Problem |
|---|---|---|
| 2024-05-10 | [ros2#1562](https://github.com/ros2/ros2/issues/1562) | QoS compatibility too strict; silent, user-hostile |
| 2024-01-31 | [Stereolabs forum](https://community.stereolabs.com/t/help-with-qos-compatibility-issue-in-zed-ros2-wrapper-and-custom-node/4483) | ZED wrapper QoS mismatch → no data, user stuck |
| 2024-01-12 | [rviz#1122](https://github.com/ros2/rviz/issues/1122) | RViz: "requesting incompatible QoS" on `/scan` |
| 2023-09-13 | [rmw_cyclonedds#473](https://github.com/ros2/rmw_cyclonedds/issues/473) | Lifespan silently not working with transient_local |
| 2023-08-29 | [rclcpp#2291](https://github.com/ros2/rclcpp/issues/2291) | Intra-process type adaptation fails *silently* on type mismatch |

## How ZeroDDS solves it

**Make the failure loud, and catch it before launch.**

- **Loud no-match events.** When an endpoint is discovered but the QoS is
  incompatible, ZeroDDS emits a `qos.incompatible.offered` /
  `qos.incompatible.requested` event naming the exact offending policy (via a
  `qos_policy_id_name` helper) instead of silently dropping the match. The
  unit test `incompatible_qos_match_emits_loud_warning` pins this behaviour.
- **Static pre-flight validation.** The `qos_check` CLI computes
  publisher/subscriber compatibility *before* you launch and exits non-zero on a
  mismatch, with the specific incompatible policy reported — so a CI job or a
  launch wrapper catches "RELIABLE vs BEST_EFFORT" the moment it is introduced,
  not after a field debugging session.
- **Right defaults so the common case just matches.** `RuntimeConfig::ros_defaults()`
  offers the representations and caps ROS writers actually use, so the
  most common silent-mismatch cause (representation/encoding) does not arise out
  of the box.

## Why it no longer has to be a pain

The pain is not that QoS has rules — it is that breaking them is *invisible*.
ZeroDDS keeps the spec-correct matching semantics (so interop is preserved) but
converts the silent no-match into a named, surfaced event and a pre-launch
check. The bug stops being "no data, no clue" and becomes "line 12: RELIABLE
requested, BEST_EFFORT offered."

## Reproduce it yourself

```bash
# Static QoS compatibility check (exit code + named offending policy):
cargo run -p zerodds-qos --example qos_check -- <writer-qos> <reader-qos>
```

The loud-warning path is covered by `incompatible_qos_match_emits_loud_warning`
in the DCPS test suite.

→ [Back to overview](index.md) · Next: [Large data](large-data.md)
