#!/bin/bash
set -u
export LD_LIBRARY_PATH=/opt/fastdds/lib
REP=${1:-2}
DOM=${2:-31}
OUT=/root/gapprobe/fastdds/fd-xcdr$REP.pcap
rm -f "$OUT"
tshark -i lo -f "udp" -w "$OUT" -q >/dev/null 2>&1 &
TSHARK=$!
sleep 3
./fd_sub $DOM 22 > sub-xcdr$REP.log 2>&1 &
SUB=$!
sleep 2
./fd_pub $DOM $REP > pub-xcdr$REP.log 2>&1
wait $SUB
sleep 1
kill -INT $TSHARK 2>/dev/null; wait $TSHARK 2>/dev/null
echo "pub: $(tail -1 pub-xcdr$REP.log)"
echo "sub read: $(tail -1 sub-xcdr$REP.log)"
