#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# CORBA perf baseline recorder — one run, all relevant features:
#   1. Cross-vendor Echo roundtrip latency (ZeroDDS hand-marshalled + codegen vs
#      omniORB/TAO/JacORB) across several payload sizes.
#   2. ZeroDDS feature-matrix per-operation latency (codegen).
#   3. SSLIOP/TLS overhead (plain vs established-conn TLS, same payload).
#   4. Codegen vs hand-marshalled delta.
# Loopback (127.0.0.1), GIOP/IIOP 1.2, one connection, SYNC_WITH_TARGET.
#
# Runs on the Linux test host (Debian 13). Env: N (iter, default 50000),
# PAYLOADS (default "32 256 4096").
set -uo pipefail
cd "$(dirname "$0")"
HERE="$(pwd)"
ROOT="$(cd ../../.. && pwd)"
WORK=$(mktemp -d)
trap 'kill $(jobs -p) 2>/dev/null; rm -rf "$WORK"' EXIT

N="${N:-50000}"
PAYLOADS="${PAYLOADS:-32 256 4096}"

echo "===================================================================="
echo " CORBA Perf Baseline — $(uname -n), N=$N, Payloads='$PAYLOADS'"
echo "===================================================================="

echo "== Build ZeroDDS perf binaries =="
( cd "$ROOT" && cargo build --release -p zerodds-corba-interop \
    --bin echo_bench --bin echo_bench_codegen --bin bench_features --bin ssliop_bench ) || exit 1
ZB="$ROOT/target/release/echo_bench"
ZBC="$ROOT/target/release/echo_bench_codegen"
ZBF="$ROOT/target/release/bench_features"
ZSSLB="$ROOT/target/release/ssliop_bench"

cp omniorb/Echo.idl "$WORK/Echo.idl"

# Cert for the SSLIOP bench (self-signed leaf, CA:FALSE, SAN=localhost).
openssl req -x509 -newkey rsa:2048 -keyout "$WORK/k.pem" -out "$WORK/c.pem" -days 1 -nodes \
  -subj "/CN=localhost" -addext "subjectAltName=DNS:localhost" \
  -addext "basicConstraints=critical,CA:FALSE" -addext "keyUsage=critical,digitalSignature" \
  -addext "extendedKeyUsage=serverAuth" >/dev/null 2>&1

# --- Vendor-Builds ----------------------------------------------------------
build_omni() {
  command -v omniidl >/dev/null || return 1
  ( cd "$WORK" && omniidl -bcxx Echo.idl ) >/dev/null 2>&1 || return 1
  local L="-lomniORB4 -lomnithread -lomniDynamic4 -lpthread"
  g++ -O2 -std=c++17 -I"$WORK" "$HERE/omniorb/server.cc" "$WORK/EchoSK.cc" -o "$WORK/omni_srv" $L 2>/dev/null || return 1
  g++ -O2 -std=c++17 -I"$WORK" "$HERE/omniorb/client.cc" "$WORK/EchoSK.cc" -o "$WORK/omni_cli" $L 2>/dev/null || return 1
}
TAO_PREFIX=/opt/opendds-secure
build_tao() {
  [ -x "$TAO_PREFIX/bin/tao_idl" ] || return 1
  export ACE_ROOT="$TAO_PREFIX/share/ace" TAO_ROOT="$TAO_PREFIX/share/tao"
  export LD_LIBRARY_PATH="$TAO_PREFIX/lib:${LD_LIBRARY_PATH:-}"
  ( cd "$WORK" && "$TAO_PREFIX/bin/tao_idl" -I"$TAO_PREFIX/include" Echo.idl ) >/dev/null 2>&1 || return 1
  local C="-O2 -std=c++17 -I$WORK -I$TAO_PREFIX/include"
  local L="-L$TAO_PREFIX/lib -lTAO_PortableServer -lTAO_AnyTypeCode -lTAO -lACE -lpthread"
  g++ $C "$HERE/tao/server.cpp" "$WORK/EchoC.cpp" "$WORK/EchoS.cpp" -o "$WORK/tao_srv" $L 2>/dev/null || return 1
  g++ $C "$HERE/tao/client.cpp" "$WORK/EchoC.cpp" "$WORK/EchoS.cpp" -o "$WORK/tao_cli" $L 2>/dev/null || return 1
}
JAVA=/opt/jdk8/bin/java; JAVAC=/opt/jdk8/bin/javac; JACORB=/opt/jacorb; JCP="$JACORB/lib/*"
JPROPS="-Dorg.omg.CORBA.ORBClass=org.jacorb.orb.ORB -Dorg.omg.CORBA.ORBSingletonClass=org.jacorb.orb.ORBSingleton"
build_jacorb() {
  [ -x "$JACORB/bin/idl" ] && [ -x "$JAVAC" ] || return 1
  ( cd "$WORK" && "$JACORB/bin/idl" Echo.idl ) >/dev/null 2>&1 || return 1
  ( cd "$WORK" && "$JAVAC" -encoding UTF-8 -cp "$JCP" -d . *.java "$HERE/jacorb/Server.java" "$HERE/jacorb/Client.java" ) >/dev/null 2>&1 || return 1
}

BO=0; BT=0; BJ=0
build_omni   && BO=1 || echo "  (omniORB build n/a — skipped)"
build_tao    && BT=1 || echo "  (TAO build n/a — skipped)"
build_jacorb && BJ=1 || echo "  (JacORB build n/a — skipped)"

# --- 1. Cross-vendor Echo latency ------------------------------------------
echo
echo "################ 1. Cross-Vendor Echo-Roundtrip-Latenz ################"
for P in $PAYLOADS; do
  echo
  echo "---- payload = ${P} B ----"
  "$ZB"  "$P" "$N"
  "$ZBC" "$P" "$N"
  if [ "$BO" = 1 ]; then
    rm -f /tmp/echo_omni.ior; "$WORK/omni_srv" >/dev/null 2>&1 &
    for _ in $(seq 1 50); do [ -s /tmp/echo_omni.ior ] && break; sleep 0.1; done
    "$WORK/omni_cli" "$P" "$N"; kill %1 2>/dev/null; wait %1 2>/dev/null
  fi
  if [ "$BT" = 1 ]; then
    rm -f /tmp/echo_tao.ior; "$WORK/tao_srv" >/dev/null 2>&1 &
    for _ in $(seq 1 50); do [ -s /tmp/echo_tao.ior ] && break; sleep 0.1; done
    "$WORK/tao_cli" "$P" "$N"; kill %1 2>/dev/null; wait %1 2>/dev/null
  fi
  if [ "$BJ" = 1 ]; then
    rm -f /tmp/echo_jacorb.ior
    ( cd "$WORK" && "$JAVA" $JPROPS -cp ".:$JCP" Server ) >/dev/null 2>&1 &
    for _ in $(seq 1 100); do [ -s /tmp/echo_jacorb.ior ] && break; sleep 0.1; done
    ( cd "$WORK" && "$JAVA" $JPROPS -cp ".:$JCP" Client "$P" "$N" )
    kill %1 2>/dev/null; wait %1 2>/dev/null
  fi
done

# --- 2. ZeroDDS feature matrix per operation -------------------------------
echo
echo "################ 2. ZeroDDS Feature-Matrix (Codegen) ################"
"$ZBF" "$N"

# --- 3. SSLIOP/TLS overhead -------------------------------------------------
echo
echo "################ 3. SSLIOP/TLS-Overhead (56 B, established conn) ################"
"$ZBC" 56 "$N"
"$ZSSLB" "$WORK/c.pem" "$WORK/k.pem" 56 "$N"

echo
echo "===================================================================="
echo " Perf baseline run done."
echo "===================================================================="
