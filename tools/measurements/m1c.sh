set -u
export DDS_ROOT=/opt/opendds
[ -f /opt/opendds/share/dds/dds-devel.sh ] && . /opt/opendds/share/dds/dds-devel.sh 2>/dev/null || true
ls /opt/opendds/share/dds/ 2>/dev/null | head
cd /root/m1-bitbound; mkdir -p od8 od16
for b in 8 16; do
  echo "##### opendds_idl bb$b"
  (cd od$b && /opt/opendds/bin/opendds_idl -Sa -St -I/opt/opendds -o . ../bb$b.idl 2>&1 | tail -6; ls)
done
echo "### od8 enum + serializer"
grep -n -B2 -A8 "Colour" od8/*.h 2>/dev/null | head -40
grep -n "Colour" od8/*.cpp 2>/dev/null | head -30
