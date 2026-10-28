#!/bin/bash
set -u
export NDDSHOME=/opt/rti.com/rti_connext_dds-7.7.0
export LD_LIBRARY_PATH=$NDDSHOME/lib/x64Linux4gcc8.5.0
export NDDS_DISCOVERY_PEERS=udpv4://127.0.0.1
REP=${1:-2}
DOM=${2:-11}
OUT=/root/gapprobe/rti-alt-xcdr$REP.pcap
rm -f "$OUT"
tshark -i lo -f "udp" -w "$OUT" -q >/dev/null 2>&1 &
TSHARK=$!
sleep 3
./gap_sub $DOM 26 > sub2-xcdr$REP.log 2>&1 &
SUB=$!
sleep 2
./gap_pub2 $DOM $REP > pub2-xcdr$REP.log 2>&1
wait $SUB
sleep 1
kill -INT $TSHARK 2>/dev/null
wait $TSHARK 2>/dev/null
grep -v "RTI LICENSE\|Connext license\|maximum of\|evaluations@\|^$" pub2-xcdr$REP.log
grep -c "^read" sub2-xcdr$REP.log
