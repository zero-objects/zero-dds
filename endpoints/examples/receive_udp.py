#!/usr/bin/env python
# SPDX-License-Identifier: Apache-2.0
# Example receiver: a pure-Python endpoint receives a DATA message the hub
# pushes over UDP and decodes the SensorReading (ADR 0013).
#   python receive_udp.py <port>
from __future__ import print_function
import os, socket, sys
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "python"))
import zerodds_wire as zw
import zerodds_endpoint as ze


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 7447
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    s.bind(("0.0.0.0", port))
    print("python receiver: listening on udp/%d" % port)
    data, _ = s.recvfrom(2048)
    body = ze.xrce_read_frame(data)
    r = zw.Reader(body, zw.LE)
    sample = {
        "id": r.get_u32(), "kind": r.get_u16(), "flags": r.get_u8(),
        "value": r.get_f32(), "stamp": r.get_u64(), "label": r.get_string(),
        "raw": r.get_seq_u8(),
    }
    assert sample["id"] == 0xA1B2C3D4 and sample["label"] == u"bay-12"
    print("PYTHON RECEIVER OK: id=0x%08X label=%s value=%g"
          % (sample["id"], sample["label"], sample["value"]))
    s.close()


if __name__ == "__main__":
    main()
