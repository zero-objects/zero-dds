#!/usr/bin/env python3
"""The foreign side of the WebSocket bridge scenario: the Python `websockets`
library, an RFC 6455 client this repository did not write.

    ws_peer.py <out-url> <count> <in-url> <hex>...

Opens both connections and prints `connected`; prints `frame <hex>` for each
of the first <count> binary frames the out-connection receives; sends each
<hex> as one binary frame on the in-connection and prints `sent`; then waits
until its standard input ends.
"""
import sys

from websockets.sync.client import connect


def main(argv):
    out_url, count, in_url, payloads = argv[1], int(argv[2]), argv[3], argv[4:]
    with connect(out_url, open_timeout=10) as out, connect(in_url, open_timeout=10) as inbound:
        print("connected", flush=True)
        for _ in range(count):
            frame = out.recv(timeout=20)
            if isinstance(frame, str):
                print("text " + frame, flush=True)
            else:
                print("frame " + frame.hex(), flush=True)
        for payload in payloads:
            inbound.send(bytes.fromhex(payload))
        print("sent", flush=True)
        sys.stdin.read()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
