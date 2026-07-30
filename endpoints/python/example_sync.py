#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Deep example (sync): a realistic sensor-telemetry flow. A publisher frames
# five typed Reading(id, value, label) samples and delivers them; the subscriber
# owns the run-loop and polls, decoding EVERY field byte-for-byte.

import sys

import zerodds_wire as zw
import zerodds_endpoint as ze


class Reading(object):
    __slots__ = ("id", "value", "label")

    def __init__(self, id, value, label):
        self.id = id
        self.value = value
        self.label = label

    def marshal(self, endian):
        w = zw.Writer(endian)
        w.put_u32(self.id)
        w.put_f32(self.value)
        w.put_string(self.label)
        return w.bytes()

    @staticmethod
    def decode(body):
        r = zw.Reader(body, zw.LE)
        return Reading(r.get_u32(), r.get_f32(), r.get_string())


def main():
    total = 5
    transport = ze.MemTransport()
    client = ze.Client(transport)
    for i in range(total):
        r = Reading(0x1000 + i, 20.0 + i * 0.5, "bay-%02d" % i)
        client.write(r.marshal(zw.LE))

    got = 0
    while got < total:
        body = client.poll()
        if body is None:
            break
        r = Reading.decode(body)
        print('sync reading %d: id=0x%x value=%.1f label="%s"'
              % (got, r.id, r.value, r.label))
        got += 1

    if got != total:
        sys.stderr.write("incomplete: got %d of %d\n" % (got, total))
        return 1
    print("ALL OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
