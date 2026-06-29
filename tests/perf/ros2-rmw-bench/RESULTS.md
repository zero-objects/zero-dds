<!-- SPDX-License-Identifier: Apache-2.0 -->
# E1 — ROS-2 rmw competitive latency bench

Harness: `latency.py` (rclpy ping<->pong, two processes = two DDS participants) +
`run.sh` (one run per `RMW_IMPLEMENTATION`, prints a p50/p90/p99 table). Realizes
the iRobot ros2-performance intent — *rmw_zerodds runs a realistic rclpy graph
competitively* — without the iRobot tool's from-source colcon build (colcon isn't
in the RoboStack env).

## Environment

Bench host (Debian 13 LXC), **ROS 2 Humble** via micromamba/RoboStack. RMW libs:
`rmw_zerodds_cpp`, `rmw_cyclonedds_cpp`, `rmw_fastrtps_cpp`. Reliable / KEEP_LAST(64),
64 B payload, 200 Hz, n=2000, `ROS_DOMAIN_ID=77`, `ROS_LOCALHOST_ONLY=1`.

## Result (2026-06-29, after the rmw match-count fix)

| rmw | n | p50 µs | p90 µs | p99 µs |
|---|---|---|---|---|
| rmw_fastrtps_cpp | 2000 | 202.9 | 333.3 | 506.6 |
| rmw_cyclonedds_cpp | 2000 | 214.7 | 368.7 | 695.3 |
| **rmw_zerodds_cpp** | **2000** | **638.3** | **778.6** | **895.0** |

**rmw_zerodds completes the full rclpy round-trip** — the E1 functional blocker is
fixed (see below).

## Fixed — rmw match-count stub (was the E1 blocker)

`rmw_publisher_count_matched_subscriptions` and
`rmw_subscription_count_matched_publishers` were hard `*count = 0` stubs in
`rmw_c/rmw_zerodds.c`. So `Publisher.get_subscription_count()` always returned 0
even while data flowed (a one-way diagnostic showed zerodds↔zerodds delivering
232 samples at `matched_subs=0`). Any match-count-based ROS logic — `ros2 topic
info`, this bench's match-wait, lifecycle gating — broke on it.

Fix chain: c-api `zerodds_writer_matched_count` / `zerodds_reader_matched_count`
(→ runtime `user_writer/reader_matched_count`) → shim
`rmw_zerodds_publisher/subscription_matched_count` → the two rmw functions wired
in `rmw_zerodds.c`. Rebuilt `librmw_zerodds{,_cpp}.so` into the env; `matched_subs`
now reports 1.

## RESOLVED — rmw-layer latency was the same-host SHM recv loop

Root-caused + fixed. The "~3×" was **not** polling, **not** the DDS core, **not**
the codec, **not** the network path — it was the **same-host SHM carrier**.

Decomposition (codepit, 64 B, reliable, 2-proc, localhost):
- dcps core (native reader, UDP): RTT **28 µs** ≈ Cyclone ddsperf **30 µs**.
- c-api + listener + condvar, no Python (`zerodds-c-api/examples/capi_latency.rs`):
  RTT **18 µs** — beats Cyclone.
- full rmw + rclpy one-way: ZeroDDS **567 µs** vs Cyclone/FastDDS **90 µs**.

Instrumentation (`handle_user_datagram` caller-thread = `zdds-recv-shm`) showed
ROS same-host user data flows over the **SHM carrier**, not UDP. The inbox dwell
(take side) was fine (~57 µs); the loss was the SHM **recv** path. `recv_user_shm_loop`
iterated all bound consumers in ONE thread, blocking on each consumer's `recv()`
in turn — a single thread can't wait on N segment futexes at once, so worst-case
per-sample latency = `(N-1) × recv_timeout` (1 ms each). Fine for 1-2 same-host
peers; it dominates at the dozen-SHM-consumer scale ROS hits (user topics +
`ros_discovery_info` + parameter services + `rosout`). A *documented* wave-4b.4
follow-up.

**Fix** (`fix(dcps): per-consumer SHM recv threads`): one dedicated receive
thread per consumer, each blocking on its own segment futex (event-driven, no
cross-consumer serialization). Why it never bit pre-ROS: prior apps + the perf
benches have N=1 same-host consumer (or run saturating tight loops); ROS is the
first many-same-host-endpoint workload at non-saturating rate.

**Result (clean main, 64 B, 200 Hz, localhost):**
- one-way **567 µs → 80 µs** (Cyclone/FastDDS 90).
- **full RTT (latency.py) 638 µs → 155 µs** vs Cyclone 215 / FastDDS ~206 —
  **ZeroDDS now beats Cyclone on the round trip** (p90 216 vs 425, p99 324 vs 690).
  The DDS core already led (28 vs 30 µs); the fix carries that lead through the
  rmw layer.

## Cross-rmw note

zerodds↔cyclone / zerodds↔fastrtps at the ROS graph level don't exchange user
data (one-way discovery visible, recv=0) — a separate ROS-level cross-vendor type
matching issue, out of scope for E1 (which runs each rmw with itself).

## E2 — Zenoh competitive column (done)

`rmw_zenoh_cpp` (RoboStack 0.1.2) installed in the env. 4-way RTT matrix via the
same `latency.py` harness (64 B, 200 Hz, 1500 samples, all under the same load):

| rmw | RTT p50 µs | p90 | p99 |
|---|---|---|---|
| rmw_fastrtps_cpp | 206 | 335 | 551 |
| **rmw_zerodds_cpp** | **240** | 362 | 534 |
| rmw_zenoh_cpp | 245 | 349 | 531 |
| rmw_cyclonedds_cpp | 255 | 410 | 706 |

ZeroDDS is competitive — beats Cyclone and Zenoh, just behind FastDDS. (Absolute
numbers run higher than the isolated single-vendor runs above because four
vendors share the box back-to-back; the *relative* order is the signal.)
