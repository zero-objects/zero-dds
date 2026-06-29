#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# E1 — ROS-2 rmw competitive latency/throughput micro-benchmark.
#
# A realistic two-participant ROS-2 graph (ping <-> pong, separate processes =
# separate DDS participants) over whatever RMW_IMPLEMENTATION is set. Mirrors the
# intent of the iRobot ros2-performance benchmark — prove rmw_zerodds runs a real
# rclpy graph competitively against rmw_cyclonedds / rmw_fastrtps — without the
# iRobot tool's from-source colcon build.
#
#   pong: subscribes /bench_ping, republishes the same payload on /bench_pong.
#   ping: publishes a seq-stamped std_msgs/String on /bench_ping at a fixed rate,
#         matches the echo on /bench_pong, records the round-trip time (send time
#         is kept locally per seq, so no clock crosses the wire).
#
# Usage:  RMW_IMPLEMENTATION=rmw_zerodds_cpp python3 latency.py {ping|pong} \
#             [--samples N] [--rate HZ] [--payload BYTES]
#
# `ping` prints one machine-readable result line:
#   RESULT rmw=<impl> n=<recv> p50=<us> p90=<us> p99=<us> rate_hz=<achieved>

import argparse
import os
import time

import rclpy
from rclpy.node import Node
from rclpy.qos import QoSProfile, ReliabilityPolicy, HistoryPolicy
from std_msgs.msg import String

TOPIC_PING = "bench_ping"
TOPIC_PONG = "bench_pong"


def qos() -> QoSProfile:
    return QoSProfile(
        reliability=ReliabilityPolicy.RELIABLE,
        history=HistoryPolicy.KEEP_LAST,
        depth=64,
    )


def now_ns() -> int:
    return time.perf_counter_ns()


class Pong(Node):
    def __init__(self) -> None:
        super().__init__("bench_pong")
        self.pub = self.create_publisher(String, TOPIC_PONG, qos())
        self.sub = self.create_subscription(String, TOPIC_PING, self._echo, qos())

    def _echo(self, msg: String) -> None:
        self.pub.publish(msg)


class Ping(Node):
    def __init__(self, payload: int) -> None:
        super().__init__("bench_ping")
        self.pad = "x" * max(0, payload - 16)
        self.sent: dict[int, int] = {}
        self.rtts_us: list[float] = []
        self.pub = self.create_publisher(String, TOPIC_PING, qos())
        self.sub = self.create_subscription(String, TOPIC_PONG, self._recv, qos())
        self.seq = 0

    def _recv(self, msg: String) -> None:
        try:
            seq = int(msg.data[:16])
        except ValueError:
            return
        t0 = self.sent.pop(seq, None)
        if t0 is not None:
            self.rtts_us.append((now_ns() - t0) / 1000.0)

    def send_one(self) -> None:
        seq = self.seq
        self.seq += 1
        m = String()
        m.data = f"{seq:016d}{self.pad}"
        self.sent[seq] = now_ns()
        self.pub.publish(m)


def run_pong() -> None:
    rclpy.init()
    node = Pong()
    try:
        rclpy.spin(node)
    except KeyboardInterrupt:
        pass
    finally:
        node.destroy_node()
        if rclpy.ok():
            rclpy.shutdown()


def run_ping(samples: int, rate: float, payload: int, warmup: int) -> None:
    rclpy.init()
    node = Ping(payload)
    impl = os.environ.get("RMW_IMPLEMENTATION", "?")
    period = 1.0 / rate

    # Wait until pong is matched both ways: it subscribes our /bench_ping and
    # publishes /bench_pong.
    deadline = time.time() + 30.0
    while time.time() < deadline:
        rclpy.spin_once(node, timeout_sec=0.05)
        if node.pub.get_subscription_count() >= 1 and node.count_publishers(TOPIC_PONG) >= 1:
            break
    if node.pub.get_subscription_count() < 1:
        print(f"RESULT rmw={impl} n=0 p50=- p90=- p99=- rate_hz=0 ERROR=no_match")
        node.destroy_node()
        rclpy.shutdown()
        return

    total = warmup + samples
    start = time.time()
    next_send = time.perf_counter()
    sent = 0
    end_deadline = start + total * period + 10.0
    while (sent < total or node.sent) and time.time() < end_deadline:
        nowp = time.perf_counter()
        if sent < total and nowp >= next_send:
            node.send_one()
            sent += 1
            next_send += period
        rclpy.spin_once(node, timeout_sec=0.001)
    elapsed = time.time() - start

    rtts = sorted(node.rtts_us[warmup:]) if len(node.rtts_us) > warmup else sorted(node.rtts_us)
    if rtts:
        def pct(p: float) -> float:
            return rtts[min(len(rtts) - 1, int(p * len(rtts)))]
        ach = len(node.rtts_us) / elapsed if elapsed > 0 else 0.0
        print(
            f"RESULT rmw={impl} n={len(rtts)} "
            f"p50={pct(0.50):.1f} p90={pct(0.90):.1f} p99={pct(0.99):.1f} rate_hz={ach:.0f}"
        )
    else:
        print(f"RESULT rmw={impl} n=0 p50=- p90=- p99=- rate_hz=0 ERROR=no_rtt")
    node.destroy_node()
    rclpy.shutdown()


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("role", choices=["ping", "pong"])
    ap.add_argument("--samples", type=int, default=2000)
    ap.add_argument("--rate", type=float, default=200.0)
    ap.add_argument("--payload", type=int, default=64)
    ap.add_argument("--warmup", type=int, default=200)
    a = ap.parse_args()
    if a.role == "pong":
        run_pong()
    else:
        run_ping(a.samples, a.rate, a.payload, a.warmup)


if __name__ == "__main__":
    main()
