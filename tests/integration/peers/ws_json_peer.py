#!/usr/bin/env python3
"""The foreign side of the WebSocket bridge's JSON scenario: the Python
`websockets` library, an RFC 6455 client this repository did not write,
asking for the JSON text frames (`zerodds-ws-bridge-1.0+json`,
zerodds-ws-bridge-1.0 §4.1, §4.3).

    ws_json_peer.py <out-url> <count> <in-url> <json>...

Opens both connections with the subprotocol and prints `connected` and the
subprotocol each connection was given; prints `text <json>` for each of the
first <count> text frames the out-connection receives, the object with
sorted keys and `timestamp_ns` written as `int` when it is an integer (its
value is the writer's clock); sends each <json> as one text frame on the
in-connection and prints `sent`; then waits until its standard input ends.
"""
import json
import sys

from websockets.sync.client import connect

SUBPROTOCOL = "zerodds-ws-bridge-1.0+json"


def main(argv):
    out_url, count, in_url, messages = argv[1], int(argv[2]), argv[3], argv[4:]
    with connect(out_url, open_timeout=10, subprotocols=[SUBPROTOCOL]) as out, connect(
        in_url, open_timeout=10, subprotocols=[SUBPROTOCOL]
    ) as inbound:
        print("connected", flush=True)
        print(f"subprotocols {out.subprotocol} {inbound.subprotocol}", flush=True)
        for _ in range(count):
            frame = out.recv(timeout=20)
            if not isinstance(frame, str):
                print("binary " + frame.hex(), flush=True)
                continue
            obj = json.loads(frame)
            if isinstance(obj.get("timestamp_ns"), int):
                obj["timestamp_ns"] = "int"
            print("text " + json.dumps(obj, sort_keys=True, separators=(",", ":")), flush=True)
        for message in messages:
            inbound.send(message)
        print("sent", flush=True)
        sys.stdin.read()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
