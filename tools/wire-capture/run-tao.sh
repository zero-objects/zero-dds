#!/bin/bash
set -u
export LD_LIBRARY_PATH=/opt/opendds/lib
OUT=/root/gapprobe/tao/tao-giop.pcap
rm -f "$OUT" ior.txt
tshark -i lo -f "tcp port 12346" -w "$OUT" -q >/dev/null 2>&1 &
TSHARK=$!
sleep 3
./server -ORBEndpoint iiop://127.0.0.1:12346 > server.log 2>&1 &
SRV=$!
sleep 2
./client "$(cat ior.txt)" > client.log 2>&1
sleep 1
kill $SRV 2>/dev/null; wait $SRV 2>/dev/null
sleep 1
kill -INT $TSHARK 2>/dev/null; wait $TSHARK 2>/dev/null
tail -1 client.log; grep -c "^put" server.log
