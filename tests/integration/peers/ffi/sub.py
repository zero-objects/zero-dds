# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""The Python subscriber of tests/integration/tests/bindings_ffi.rs: the
ctypes loader `zerodds.loader` over libzerodds (zerodds-ffi-loader-1.0
§3.1), with `ZERODDS_LIB` naming the library.

    python3 sub.py <domain> <topic> <type name> <samples>

The protocol of sub.c: `EVENT matched`, `EVENT sample <hex>` per body,
`EVENT received n=<count>`, one more take after standard input closed,
`EVENT done`.
"""
import sys
import time

from zerodds import loader


def take_one(reader):
    body = reader.take()
    if body is None:
        return False
    print("EVENT sample " + body.hex(), flush=True)
    return True


def main(argv):
    if len(argv) != 5:
        print(f"usage: {argv[0]} <domain> <topic> <type name> <samples>", file=sys.stderr)
        return 2
    domain, topic, type_name, wanted = int(argv[1]), argv[2], argv[3], int(argv[4])
    with loader.Runtime(domain_id=domain) as runtime:
        with loader.Reader(runtime, topic, type_name, reliable=True) as reader:
            try:
                reader.wait_for_matched(1, 10000)
            except loader.ZeroDdsError:
                print("EVENT unmatched", flush=True)
                return 1
            print("EVENT matched", flush=True)
            taken = 0
            deadline = time.monotonic() + 10.0
            while taken < wanted and time.monotonic() < deadline:
                if take_one(reader):
                    taken += 1
                else:
                    # The reader has no blocking take.
                    time.sleep(0.005)
            print(f"EVENT received n={taken}", flush=True)
            sys.stdin.read()
            while take_one(reader):
                pass
            print("EVENT done", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
