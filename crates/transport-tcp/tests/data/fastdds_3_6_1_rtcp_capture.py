# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Captures the octets of fastdds_3_6_1_rtcp_bind_request.hex and
# fastdds_3_6_1_rtcp_listener_answers.hex (zerodds-tcp-transport-1.0 §6.1):
# what Fast DDS writes first on a TCP connection it opens (client role), and
# what a Fast DDS listener answers to a BindConnectionRequest and to two
# OpenLogicalPortRequests (server role). FASTDDS_SHAPES names the shapes
# client of tests/interop/ci/fastdds built against Fast DDS 3.6.1; the
# captures go to the directory given as the first argument.
#
#   FASTDDS_SHAPES=.../fastdds_shapes python3 fastdds_3_6_1_rtcp_capture.py /tmp
import os, socket, struct, subprocess, sys, time
B = os.environ["FASTDDS_SHAPES"]
OUT = (sys.argv[1] if len(sys.argv) > 1 else ".") + "/"
# Client role: Fast DDS connects to us.
srv = socket.socket(); srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
srv.bind(("127.0.0.1", 0)); srv.listen(1); port = srv.getsockname()[1]
p = subprocess.Popen([B, "sub", "Square", "7", "reliable", "volatile", "tcp", "0", "127.0.0.1:%d" % port],
                     stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
srv.settimeout(5); c, _ = srv.accept(); c.settimeout(1.5)
data = b""
try:
    while True:
        d = c.recv(65536)
        if not d: break
        data += d
except socket.timeout:
    pass
p.kill(); p.wait(); c.close(); srv.close()
open(OUT + "client-capture.bin", "wb").write(data)
print("client role:", len(data), "octets:", data[:80].hex())
# Server role: a Fast DDS listener answers our BindConnectionRequest.
p = subprocess.Popen([B, "sub", "Square", "7", "reliable", "volatile", "tcp", "47201"],
                     stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
time.sleep(1.0)
s = socket.create_connection(("127.0.0.1", 47201)); s.settimeout(1.5)
body = bytes([1, 0, 1, 0x0f]) + struct.pack("<iI", 1, 47999) + bytes(12) + bytes([127, 0, 0, 1])
payload = struct.pack("<HI", 1, 4 + len(body)) + bytes([0, 1, 0, 0]) + body
ctrl = bytes([0xD1, 0x0E]) + struct.pack("<H", 16 + len(payload)) + struct.pack("<III", 7, 0, 0) + payload
def crc(bs):
    c = 0
    for b in bs:
        c += b
        if c > 0xFFFFFFFF: c -= 0xFFFFFFFF
    return c
msg = b"RTCP" + struct.pack("<IIH", 14 + len(ctrl), crc(ctrl), 0) + ctrl
s.sendall(msg)
answer = b""
try:
    while True:
        d = s.recv(65536)
        if not d: break
        answer += d
except socket.timeout:
    pass
# Ask for the logical port 7410 + 250*7 + 10 (participant id 0 of domain 7) and 1 (none).
for tx, lp in ((8, 7410 + 1750), (9, 1)):
    body = struct.pack("<H", lp)
    payload = struct.pack("<HI", 1, 4 + len(body)) + bytes([0, 1, 0, 0]) + body
    ctrl = bytes([0xD2, 0x0E]) + struct.pack("<H", 16 + len(payload)) + struct.pack("<III", tx, 0, 0) + payload
    s.sendall(b"RTCP" + struct.pack("<IIH", 14 + len(ctrl), crc(ctrl), 0) + ctrl)
try:
    while True:
        d = s.recv(65536)
        if not d: break
        answer += d
except socket.timeout:
    pass
p.kill(); p.wait(); s.close()
open(OUT + "server-capture.bin", "wb").write(answer)
print("server role:", len(answer), "octets:", answer.hex())
