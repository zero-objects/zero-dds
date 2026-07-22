#!/usr/bin/env python
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Example: a pure-Python endpoint publishes a SensorReading to a ZeroDDS/XRCE
# hub over UDP (ADR 0013). Run the agent (endpoints/xrce-agent-demo) first.
#   python publish_udp.py <host> <port>
from __future__ import print_function
import os, socket, sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "python"))
import zerodds_wire as zw
import zerodds_endpoint as ze


def main():
    host = sys.argv[1] if len(sys.argv) > 1 else "127.0.0.1"
    port = int(sys.argv[2]) if len(sys.argv) > 2 else 7447

    # encode the sample (a fixed SensorReading)
    w = zw.Writer(zw.LE)
    w.put_u32(0xA1B2C3D4)
    w.put_u16(0x1234)
    w.put_u8(0x5A)
    w.put_f32(3.5)
    w.put_u64(0x0102030405060708)
    w.put_string(u"bay-12")
    w.put_seq_u8(bytearray([0xDE, 0xAD, 0xBE, 0xEF]))

    # frame as XRCE WRITE_DATA and send over UDP (the frame-hook, UDP fill)
    frame = ze.xrce_write_frame(ze.XRCE_SESSION_NOKEY, ze.XRCE_STREAM_BEST_EFFORT, 1, w.bytes())
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    s.sendto(frame, (host, port))
    s.close()
    print("python endpoint: sent %d-byte XRCE frame to %s:%d" % (len(frame), host, port))


if __name__ == "__main__":
    main()
