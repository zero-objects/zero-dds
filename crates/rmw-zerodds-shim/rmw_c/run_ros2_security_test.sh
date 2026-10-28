#!/usr/bin/env bash
# SROS2 through rmw_zerodds_cpp: the security options rcl resolves
# (ROS_SECURITY_ENABLE, ROS_SECURITY_STRATEGY, ROS_SECURITY_KEYSTORE and the
# node's --enclave) must reach the RMW. Before, the RMW ignored them and a
# node started with ROS security switched on ran plaintext.
#
#   1  zerodds talker -> zerodds listener, both Enforce with the keystore:
#      the listener hears the talker.
#   2  Counter-check: the same talker, a zerodds listener without security:
#      nothing is heard (the sros2 governance protects discovery and data).
#   3  zerodds talker -> rmw_cyclonedds_cpp listener, both secured with the
#      same keystore: the listener hears the talker (cross-vendor SROS2).
#
# The RMW is built with `--features security`.
#
# Usage:
#   ROS_PREFIX=$CONDA_PREFIX ./run_ros2_security_test.sh
#
# Env knobs: REPO, ZERODDS_TARGET (cargo release dir), ROS_DOMAIN_ID
# (default 51), LISTEN_SECONDS (default 12), CYCLONE_VERBOSITY (Cyclone's
# tracing level for the rmw_cyclonedds_cpp listener, default warning),
# KEEP_DIR (keep the work directory with keystore and logs: 1).
# NB: no `set -u` — ROS's setup.bash references unset vars.
set -eo pipefail

: "${ROS_PREFIX:?set ROS_PREFIX to the ROS 2 install prefix (e.g. \$CONDA_PREFIX or /opt/ros/humble)}"
HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="${REPO:-$(cd "$HERE/../../.." && pwd)}"
ZERODDS_TARGET="${ZERODDS_TARGET:-$REPO/target/release}"

set +e
# shellcheck disable=SC1091
source "$ROS_PREFIX/setup.bash" >/dev/null 2>&1 || true
set -e

echo "== 1) Rust DDS bridge + ZeroDDS C-API, with DDS-Security =="
( cd "$REPO" && cargo build -p rmw-zerodds-shim -p zerodds-c-api --release \
    --features rmw-zerodds-shim/security,zerodds-c-api/security )

echo "== 2) ABI-correct rmw layer (librmw_zerodds_cpp.so) =="
ROS_PREFIX="$ROS_PREFIX" ZERODDS_TARGET="$ZERODDS_TARGET" \
  bash "$HERE/build_librmw_zerodds_cpp.sh"

echo "== 3) register into the ROS install =="
cp "$HERE/librmw_zerodds_cpp.so" "$ROS_PREFIX/lib/"
for _lib in librmw_zerodds.so libzerodds.so; do
  [ -f "$ZERODDS_TARGET/$_lib" ] && cp "$ZERODDS_TARGET/$_lib" "$ROS_PREFIX/lib/"
done
mkdir -p "$ROS_PREFIX/share/ament_index/resource_index/rmw_implementation"
touch "$ROS_PREFIX/share/ament_index/resource_index/rmw_implementation/rmw_zerodds_cpp"

export LD_LIBRARY_PATH="$ZERODDS_TARGET:$ROS_PREFIX/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export ROS_DOMAIN_ID="${ROS_DOMAIN_ID:-51}"
DEMO="$ROS_PREFIX/lib/demo_nodes_py"
LISTEN="${LISTEN_SECONDS:-12}"
TMP="$(mktemp -d)"
if [ "${KEEP_DIR:-0}" = 1 ]; then
  echo "work directory: $TMP"
  trap 'kill $(jobs -p) 2>/dev/null || true' EXIT
else
  trap 'kill $(jobs -p) 2>/dev/null || true; rm -rf "$TMP"' EXIT
fi
rc=0

echo "== 4) keystore and enclave (SROS2 layout, made with openssl) =="
# `ros2 security create_keystore` of sros2 0.10.6 fails on the Linux test host with
# "TypeError: curve must be an EllipticCurve instance" (the installed
# `cryptography` is newer than it). The files are what sros2 writes:
# keystore/enclaves/<name>/{identity_ca.cert.pem,cert.pem,key.pem,
# permissions_ca.cert.pem,governance.p7s,permissions.p7s}, EC P-256,
# subject CN=<enclave>, governance with join access control and ENCRYPT,
# roots in the form sros2 writes them (no default namespace: Cyclone 0.10
# answers a default namespace with "Can not parse governance file").
ENC="$TMP/keystore/enclaves/zerodds_sros2"
mkdir -p "$ENC" "$TMP/ca"
openssl ecparam -name prime256v1 -genkey -noout -out "$TMP/ca/ca.key"
openssl req -x509 -new -nodes -key "$TMP/ca/ca.key" -days 30 \
  -addext basicConstraints=critical,CA:TRUE -addext subjectKeyIdentifier=hash \
  -subj "/CN=zerodds_sros2_ca" -out "$TMP/ca/ca.pem" 2>/dev/null
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out "$ENC/key.pem"
openssl req -new -key "$ENC/key.pem" -subj "/CN=\\/zerodds_sros2" -out "$TMP/ca/leaf.csr"
printf 'subjectKeyIdentifier=hash\nauthorityKeyIdentifier=keyid\n' > "$TMP/ca/ext.cnf"
openssl x509 -req -in "$TMP/ca/leaf.csr" -CA "$TMP/ca/ca.pem" -CAkey "$TMP/ca/ca.key" \
  -CAcreateserial -days 30 -extfile "$TMP/ca/ext.cnf" -out "$ENC/cert.pem" 2>/dev/null
cp "$TMP/ca/ca.pem" "$ENC/identity_ca.cert.pem"
cp "$TMP/ca/ca.pem" "$ENC/permissions_ca.cert.pem"
cat > "$TMP/ca/governance.xml" <<XML
<?xml version="1.0" encoding="UTF-8"?>
<dds xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:noNamespaceSchemaLocation="http://www.omg.org/spec/DDS-SECURITY/20170901/omg_shared_ca_governance.xsd">
  <domain_access_rules>
    <domain_rule>
      <domains><id>$ROS_DOMAIN_ID</id></domains>
      <allow_unauthenticated_participants>false</allow_unauthenticated_participants>
      <enable_join_access_control>true</enable_join_access_control>
      <discovery_protection_kind>ENCRYPT</discovery_protection_kind>
      <liveliness_protection_kind>ENCRYPT</liveliness_protection_kind>
      <rtps_protection_kind>SIGN</rtps_protection_kind>
      <topic_access_rules>
        <topic_rule>
          <topic_expression>*</topic_expression>
          <enable_discovery_protection>true</enable_discovery_protection>
          <enable_liveliness_protection>true</enable_liveliness_protection>
          <enable_read_access_control>true</enable_read_access_control>
          <enable_write_access_control>true</enable_write_access_control>
          <metadata_protection_kind>ENCRYPT</metadata_protection_kind>
          <data_protection_kind>ENCRYPT</data_protection_kind>
        </topic_rule>
      </topic_access_rules>
    </domain_rule>
  </domain_access_rules>
</dds>
XML
cat > "$TMP/ca/permissions.xml" <<XML
<?xml version="1.0" encoding="UTF-8"?>
<dds xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:noNamespaceSchemaLocation="http://www.omg.org/spec/DDS-SECURITY/20170901/omg_shared_ca_permissions.xsd">
  <permissions>
    <grant name="/zerodds_sros2">
      <subject_name>CN=/zerodds_sros2</subject_name>
      <validity><not_before>2025-01-01T00:00:00</not_before><not_after>2099-01-01T00:00:00</not_after></validity>
      <allow_rule>
        <domains><id>$ROS_DOMAIN_ID</id></domains>
        <publish><topics><topic>*</topic></topics></publish>
        <subscribe><topics><topic>*</topic></topics></subscribe>
      </allow_rule>
      <default>DENY</default>
    </grant>
  </permissions>
</dds>
XML
for doc in governance permissions; do
  openssl smime -sign -in "$TMP/ca/$doc.xml" -text -out "$ENC/$doc.p7s" \
    -signer "$TMP/ca/ca.pem" -inkey "$TMP/ca/ca.key"
done

# An array, not a function: `func &` runs a subshell, and killing its PID
# left the python node behind, writing into the next pair's log.
SECURE_ENV=(env ROS_SECURITY_ENABLE=true ROS_SECURITY_STRATEGY=Enforce
  ROS_SECURITY_KEYSTORE="$TMP/keystore")

# pair LABEL EXPECT(pass|fail) LISTENER_RMW LISTENER_SECURE(yes|no)
pair() {
  echo "== $1 =="
  "${SECURE_ENV[@]}" RMW_IMPLEMENTATION=rmw_zerodds_cpp \
    python "$DEMO/talker" --ros-args --enclave /zerodds_sros2 > "$TMP/talker.log" 2>&1 &
  local talker=$!
  # Cyclone's own reason, if its participant does not come up.
  export CYCLONEDDS_URI="<CycloneDDS><Domain><Tracing><Verbosity>${CYCLONE_VERBOSITY:-warning}</Verbosity><OutputFile>$TMP/cyclone.log</OutputFile></Tracing></Domain></CycloneDDS>"
  if [ "$4" = yes ]; then
    "${SECURE_ENV[@]}" RMW_IMPLEMENTATION="$3" timeout "$LISTEN" \
      python "$DEMO/listener" --ros-args --enclave /zerodds_sros2 > "$TMP/listener.log" 2>&1 || true
  else
    RMW_IMPLEMENTATION="$3" timeout "$LISTEN" \
      python "$DEMO/listener" > "$TMP/listener.log" 2>&1 || true
  fi
  kill "$talker" 2>/dev/null || true
  wait "$talker" 2>/dev/null || true
  local heard
  heard=$(grep -c "I heard" "$TMP/listener.log" || true)
  echo "-- talker (tail) --"; tail -3 "$TMP/talker.log"
  echo "-- listener heard $heard message(s) --"; tail -3 "$TMP/listener.log"
  case "$2" in
    pass) [ "$heard" -gt 0 ] || { echo "FAIL: nothing heard"; rc=1; } ;;
    fail) [ "$heard" -eq 0 ] || { echo "FAIL (counter-check): a plaintext listener heard $heard"; rc=1; } ;;
  esac
  if [ "$heard" -eq 0 ] && [ "$2" = pass ] && [ -s "$TMP/cyclone.log" ]; then
    echo "-- cyclone trace (tail) --"; tail -8 "$TMP/cyclone.log"
  fi
}

pair "4a) secured zerodds -> secured zerodds" pass rmw_zerodds_cpp yes
pair "4b) counter-check: secured zerodds -> plaintext zerodds hears nothing" fail rmw_zerodds_cpp no
pair "4c) secured zerodds -> secured rmw_cyclonedds_cpp" pass rmw_cyclonedds_cpp yes
exit "$rc"
