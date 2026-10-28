#!/bin/bash
set -u
OUT=/root/gapprobe/omni/omni-giop.pcap
rm -f "$OUT" ior.txt
tshark -i lo -f "tcp port 12345" -w "$OUT" -q >/dev/null 2>&1 &
TSHARK=$!
sleep 3
./server -ORBendPoint giop:tcp:127.0.0.1:12345 > server.log 2>&1 &
SRV=$!
sleep 2
./client "$(cat ior.txt)" -ORBendPoint giop:tcp:127.0.0.1:0 > client.log 2>&1
sleep 1
kill $SRV 2>/dev/null; wait $SRV 2>/dev/null
sleep 1
kill -INT $TSHARK 2>/dev/null; wait $TSHARK 2>/dev/null
echo "--- client"; cat client.log
echo "--- server"; cat server.log
ls -la "$OUT"
