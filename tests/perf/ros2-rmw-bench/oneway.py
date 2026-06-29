#!/usr/bin/env python3
# One-way latency probe: pub embeds perf_counter_ns() in a std_msgs/String;
# sub computes (recv_ns - send_ns). Same host => CLOCK_MONOTONIC is shared, so
# the one-way delay is real. Isolates a SINGLE hop (pub->sub through rclpy/rmw)
# instead of the RTT, so we can see per-hop cost + symmetry vs the RTT bench.
import argparse
import time

import rclpy
from rclpy.node import Node
from rclpy.qos import HistoryPolicy, QoSProfile, ReliabilityPolicy
from std_msgs.msg import String

TOPIC = "oneway_probe"


def qos():
    return QoSProfile(
        reliability=ReliabilityPolicy.RELIABLE,
        history=HistoryPolicy.KEEP_LAST,
        depth=64,
    )


class Pub(Node):
    def __init__(self, payload):
        super().__init__("oneway_pub")
        self.pub = self.create_publisher(String, TOPIC, qos())
        self.pad = "x" * max(0, payload - 24)

    def send(self):
        m = String()
        m.data = f"{time.perf_counter_ns()};{self.pad}"
        self.pub.publish(m)


class Sub(Node):
    def __init__(self, target):
        super().__init__("oneway_sub")
        self.sub = self.create_subscription(String, TOPIC, self._recv, qos())
        self.delays = []
        self.target = target
        self.done = False

    def _recv(self, m):
        now = time.perf_counter_ns()
        try:
            send = int(m.data.split(";", 1)[0])
        except ValueError:
            return
        self.delays.append((now - send) / 1000.0)  # us
        if len(self.delays) >= self.target:
            self.done = True


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("role", choices=["pub", "sub"])
    ap.add_argument("--samples", type=int, default=1000)
    ap.add_argument("--rate", type=float, default=200.0)
    ap.add_argument("--payload", type=int, default=64)
    ap.add_argument("--warmup", type=int, default=100)
    a = ap.parse_args()
    rclpy.init()
    if a.role == "sub":
        node = Sub(a.warmup + a.samples)
        while rclpy.ok() and not node.done:
            rclpy.spin_once(node, timeout_sec=0.1)
        d = sorted(node.delays[a.warmup:]) if len(node.delays) > a.warmup else sorted(node.delays)
        if d:
            def pct(p):
                return d[min(len(d) - 1, int(p * len(d)))]
            print(f"ONEWAY n={len(d)} p50={pct(.5):.1f} p90={pct(.9):.1f} p99={pct(.99):.1f} min={d[0]:.1f} (us)")
        else:
            print("ONEWAY n=0")
        return
    # pub
    node = Pub(a.payload)
    deadline = time.time() + 20.0
    while time.time() < deadline and node.count_subscribers(TOPIC) < 1:
        rclpy.spin_once(node, timeout_sec=0.05)
    total = a.warmup + a.samples
    period = 1.0 / a.rate
    nxt = time.perf_counter()
    sent = 0
    while sent < total:
        nowp = time.perf_counter()
        if nowp >= nxt:
            node.send()
            sent += 1
            nxt += period
        else:
            time.sleep(min(period / 4, max(0, nxt - nowp)))
    time.sleep(1.0)  # let last samples flush
    print(f"pub done ({sent} sent)")


if __name__ == "__main__":
    main()
