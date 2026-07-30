#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Deep example (async): the same sensor-telemetry flow, but the subscriber does
# not own the run-loop. `AsyncReader.stream()` is an async generator; the
# consumer iterates it with `async for` — the idiomatic asyncio model. Every
# field is decoded.

import asyncio
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


class AsyncReader(object):
    """Idiomatic asyncio reader: an async generator over decoded sample bodies."""

    def __init__(self, transport):
        self.transport = transport

    async def stream(self):
        while True:
            frame = self.transport.receive()
            if frame is None:
                await asyncio.sleep(0.001)
                continue
            yield ze.xrce_read_frame(frame)


async def run():
    total = 5
    transport = ze.MemTransport()
    client = ze.Client(transport)
    for i in range(total):
        r = Reading(0x2000 + i, 100.0 - i, "sensor-%02d" % i)
        client.write(r.marshal(zw.LE))

    reader = AsyncReader(transport)
    got = 0
    async for body in reader.stream():
        r = Reading.decode(body)
        print('async reading %d: id=0x%x value=%.1f label="%s"'
              % (got, r.id, r.value, r.label))
        got += 1
        if got == total:
            break
    print("ALL OK")


if __name__ == "__main__":
    asyncio.run(run())
    sys.exit(0)
