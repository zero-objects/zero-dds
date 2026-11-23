#!/usr/bin/env python3
"""qpid-proton's Python binding as the AMQP 1.0 counterpart of the bridge
daemon in front of a broker (zerodds-amqp-bridge-daemon-1.0 §10): a client
this repository did not write, on one connection to the broker.

    proton_peer.py <url>

<url> is amqp://[user:password@]host:port. The program reads one command
per line from its standard input and answers each with lines on its
standard output:

    receiver <address> <credit>
        attaches a receiver from <address> and grants <credit>;
        answers `attached` once the broker attached it.
    take <address> <n> <seconds>
        up to <n> messages from <address> (a receiver attached before or
        now, credit <n>), each accepted and answered as
        `message <content-type> <body-hex> <durable> <k>=<v>,...` (the
        application properties); then `done <count>`.
    send <address> <hex>
        sends the encoded message <hex> (its sections, AMQP 1.0 Part 3
        §3.2) on a sender to <address>; answers `outcome accepted`,
        `outcome rejected <condition>`, `outcome released`,
        `outcome modified` or `outcome none` (no disposition in 10 s).
    quit
"""
import select
import sys
import time

from proton import Delivery, Message
from proton.utils import BlockingConnection


def body_hex(body):
    if isinstance(body, (bytes, bytearray, memoryview)):
        return bytes(body).hex()
    return "?" + type(body).__name__


def run_io(conn, seconds):
    """Lets the connection do its I/O for `seconds`: the dispositions of
    accepted messages go out, heartbeats keep the peer's idle time-out from
    expiring while the program waits for its next command."""
    try:
        conn.wait(lambda: False, timeout=seconds)
    except Exception:  # proton.Timeout
        pass


def main(argv):
    url = argv[1]
    conn = BlockingConnection(url, timeout=20, allowed_mechs="PLAIN ANONYMOUS",
                              allow_insecure_mechs=True)
    receivers = {}
    senders = {}
    print("connected", flush=True)
    while True:
        ready, _, _ = select.select([sys.stdin], [], [], 0.2)
        if not ready:
            run_io(conn, 0.05)
            continue
        line = sys.stdin.readline()
        if not line:
            break
        words = line.split()
        if not words:
            continue
        cmd = words[0]
        if cmd == "quit":
            break
        if cmd == "receiver":
            address, credit = words[1], int(words[2])
            receivers[address] = conn.create_receiver(address, credit=credit)
            print("attached", flush=True)
        elif cmd == "take":
            address, n, secs = words[1], int(words[2]), float(words[3])
            if address not in receivers:
                receivers[address] = conn.create_receiver(address, credit=n)
            r = receivers[address]
            deadline = time.monotonic() + secs
            got = 0
            while got < n:
                left = deadline - time.monotonic()
                if left <= 0:
                    break
                try:
                    m = r.receive(timeout=left)
                except Exception:  # proton.Timeout
                    break
                r.accept()
                props = ",".join(f"{k}={v}" for k, v in (m.properties or {}).items())
                print(f"message {m.content_type} {body_hex(m.body)} {m.durable} {props}",
                      flush=True)
                got += 1
            run_io(conn, 0.1)
            print(f"done {got}", flush=True)
        elif cmd == "send":
            address, data = words[1], bytes.fromhex(words[2])
            if address not in senders:
                senders[address] = conn.create_sender(address)
            m = Message()
            m.decode(data)
            d = senders[address].link.send(m)
            try:
                conn.wait(lambda: d.remote_state != 0 or d.settled, timeout=10)
            except Exception as e:  # proton.Timeout
                print(f"outcome none {type(e).__name__}", flush=True)
                continue
            state = d.remote_state
            if state == Delivery.ACCEPTED:
                print("outcome accepted", flush=True)
            elif state == Delivery.REJECTED:
                cond = d.remote.condition
                print(f"outcome rejected {cond.name if cond else '-'}", flush=True)
            elif state == Delivery.RELEASED:
                print("outcome released", flush=True)
            elif state == Delivery.MODIFIED:
                print("outcome modified", flush=True)
            else:
                print(f"outcome state-{state}", flush=True)
            d.settle()
        else:
            print(f"error unknown command {cmd}", flush=True)
    conn.close()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
