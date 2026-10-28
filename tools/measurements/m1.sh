set -u
W=/root/m1-bitbound
rm -rf $W; mkdir -p $W; cd $W
cat > bb8.idl <<'IDL'
@bit_bound(8)
enum Colour { RED, GREEN, BLUE };

@final struct Sample {
    Colour first;
    Colour second;
    octet tail;
};
IDL
cat > bb16.idl <<'IDL'
@bit_bound(16)
enum Colour { RED, GREEN, BLUE };

@final struct Sample {
    Colour first;
    Colour second;
    octet tail;
};
IDL

echo "##### FASTDDSGEN version"
/root/vendors/fastddsgen/scripts/fastddsgen -version 2>&1 | head -5
for b in 8 16; do
  mkdir -p fd$b; cd fd$b
  echo "##### FASTDDSGEN bb$b"
  /root/vendors/fastddsgen/scripts/fastddsgen -replace -d . ../bb$b.idl 2>&1 | tail -5
  echo "--- files:"; ls
  cd ..
done

echo "##### RTIDDSGEN version"
/opt/rti.com/rti_connext_dds-7.7.0/bin/rtiddsgen -version 2>&1 | head -5
for b in 8 16; do
  mkdir -p rti$b; cd rti$b
  echo "##### RTIDDSGEN bb$b"
  /opt/rti.com/rti_connext_dds-7.7.0/bin/rtiddsgen -language C++11 -replace -d . ../bb$b.idl 2>&1 | tail -8
  echo "--- files:"; ls
  cd ..
done

echo "##### OPENDDS_IDL version"
/opt/opendds/bin/opendds_idl --version 2>&1 | head -3
