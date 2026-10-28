#!/bin/bash
set -u
CP=$(ls /opt/jacorb/lib/*.jar | tr "\n" ":")
OUT=/root/gapprobe/jacorb/jacorb-giop.pcap
rm -f "$OUT" ior.txt
tshark -i lo -f "tcp port 12347" -w "$OUT" -q >/dev/null 2>&1 &
TSHARK=$!
sleep 3
java -cp "$CP:.:shimclasses" Server > server.log 2>&1 &
SRV=$!
sleep 5
java -cp "$CP:.:shimclasses" Client "$(cat ior.txt)" > client.log 2>&1
sleep 1
kill $SRV 2>/dev/null; wait $SRV 2>/dev/null
sleep 1
kill -INT $TSHARK 2>/dev/null; wait $TSHARK 2>/dev/null
tail -1 client.log; grep -c "^put" server.log
