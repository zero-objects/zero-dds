#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Cross-ORB interop harness: ZeroDDS <-> {omniORB, TAO, JacORB}, both
# directions, via stringified-IOR exchange + the shared interop.idl.
#
# Runs on the Linux test host (Debian 13). Four calls are checked per foreign ORB:
#   - <ORB> server   <- ZeroDDS client   (echo, bench)
#   - ZeroDDS server <- <ORB> client     (echo, bench)
# Exit 0 = all available combinations green.
set -uo pipefail
cd "$(dirname "$0")"
HERE="$(pwd)"
ROOT="$(cd ../../.. && pwd)"
WORK=$(mktemp -d)
trap 'kill $(jobs -p) 2>/dev/null; rm -rf "$WORK"' EXIT

RC=0
declare -a RESULTS

# --- ZeroDDS binaries -------------------------------------------------------
echo "== Build ZeroDDS interop binaries =="
( cd "$ROOT" && cargo build --release -p zerodds-corba-interop \
    --bin interop_server --bin interop_client \
    --bin ssliop_server --bin ssliop_client ) || exit 1
ZSRV="$ROOT/target/release/interop_server"
ZCLI="$ROOT/target/release/interop_client"
ZSSLSRV="$ROOT/target/release/ssliop_server"
ZSSLCLI="$ROOT/target/release/ssliop_client"

cp interop.idl "$WORK/"
cp cosnaming.idl "$WORK/"

# --- omniORB ----------------------------------------------------------------
build_omni() {
  command -v omniidl >/dev/null || return 1
  # -Wba: generate TypeCode/Any operators (operator<<=/>>= for AnyPair/LongSeq)
  # into interopDynSK.cc — required for aecho(struct/sequence) over any.
  ( cd "$WORK" && omniidl -bcxx -Wba interop.idl ) || return 1
  local CF="-O2 -std=c++17 -I$WORK -DOMNI_UNLOADABLE_STUBS"
  local LB="-lomniORB4 -lomnithread -lomniDynamic4 -lpthread"
  g++ $CF "$HERE/omniorb/interop_server.cc" "$WORK/interopSK.cc" "$WORK/interopDynSK.cc" -o "$WORK/omni_server" $LB || return 1
  g++ $CF "$HERE/omniorb/interop_client.cc" "$WORK/interopSK.cc" "$WORK/interopDynSK.cc" -o "$WORK/omni_client" $LB || return 1
}

# --- TAO --------------------------------------------------------------------
TAO_PREFIX=/opt/opendds-secure
build_tao() {
  [ -x "$TAO_PREFIX/bin/tao_idl" ] || return 1
  # tao_idl needs ACE_ROOT/TAO_ROOT, otherwise it generates no stubs.
  export ACE_ROOT="$TAO_PREFIX/share/ace" TAO_ROOT="$TAO_PREFIX/share/tao"
  export LD_LIBRARY_PATH="$TAO_PREFIX/lib:${LD_LIBRARY_PATH:-}"
  ( cd "$WORK" && ACE_ROOT="$ACE_ROOT" TAO_ROOT="$TAO_ROOT" \
      "$TAO_PREFIX/bin/tao_idl" -I"$TAO_PREFIX/include" interop.idl ) || return 1
  local CF="-O2 -std=c++17 -I$WORK -I$TAO_PREFIX/include"
  local LB="-L$TAO_PREFIX/lib -lTAO_PortableServer -lTAO_AnyTypeCode -lTAO -lACE -lpthread"
  g++ $CF "$HERE/tao/interop_server.cpp" "$WORK/interopC.cpp" "$WORK/interopS.cpp" -o "$WORK/tao_server" $LB || return 1
  g++ $CF "$HERE/tao/interop_client.cpp" "$WORK/interopC.cpp" "$WORK/interopS.cpp" -o "$WORK/tao_client" $LB || return 1
}

# --- JacORB -----------------------------------------------------------------
JAVA=/opt/jdk8/bin/java
JAVAC=/opt/jdk8/bin/javac
JACORB=/opt/jacorb
JCP="$JACORB/lib/*"
JPROPS="-Dorg.omg.CORBA.ORBClass=org.jacorb.orb.ORB -Dorg.omg.CORBA.ORBSingletonClass=org.jacorb.orb.ORBSingleton"
build_jacorb() {
  [ -x "$JACORB/bin/idl" ] || return 1
  [ -x "$JAVAC" ] || return 1
  ( cd "$WORK" && "$JACORB/bin/idl" interop.idl ) || return 1
  # JDK 8 javac defaults to the platform encoding (ASCII) → force UTF-8;
  # the jacorb-generated files + our sources contain non-ASCII.
  ( cd "$WORK" && "$JAVAC" -encoding UTF-8 -cp "$JCP" -d . *.java "$HERE/jacorb/InteropServer.java" "$HERE/jacorb/InteropClient.java" ) || return 1
}
jacorb_server() { ( cd "$WORK" && "$JAVA" $JPROPS -cp ".:$JCP" InteropServer ); }
jacorb_client() { ( cd "$WORK" && "$JAVA" $JPROPS -cp ".:$JCP" InteropClient "$1" "$2" ); }

ior_of() { grep "^$1=" "$2" | cut -d= -f2-; }

# Waits until the IOR file contains the BENCH_IOR line (server startup robust
# against timing races, especially TAO/JVM under build load). Max ~15s.
wait_ior() {
  for _ in $(seq 1 75); do
    grep -q "^BENCH_IOR=" "$1" 2>/dev/null && return 0
    sleep 0.2
  done
  return 1
}

# Direction A: foreign server <- ZeroDDS client
zerodds_calls() {
  local name="$1" iorfile="$2"
  echo "--- ZeroDDS-Client -> $name Echo ---"
  "$ZCLI" echo  "$(ior_of ECHO_IOR  "$iorfile")" && \
  echo "--- ZeroDDS-Client -> $name Bench ---" && \
  "$ZCLI" bench "$(ior_of BENCH_IOR "$iorfile")"
}

echo
echo "############## omniORB ##############"
if build_omni; then
  "$WORK/omni_server" > "$WORK/o.ior" 2>/dev/null & wait_ior "$WORK/o.ior"
  zerodds_calls "omniORB" "$WORK/o.ior" && RESULTS+=("omniORB-Server<-ZeroDDS: OK") || { RESULTS+=("omniORB-Server<-ZeroDDS: FAIL"); RC=1; }
  kill %1 2>/dev/null; wait %1 2>/dev/null
  "$ZSRV" 127.0.0.1 0 > "$WORK/z.ior" 2>/dev/null & wait_ior "$WORK/z.ior"
  echo "--- omniORB-Client -> ZeroDDS Echo ---";  "$WORK/omni_client" echo  "$(ior_of ECHO_IOR  "$WORK/z.ior")" && \
  echo "--- omniORB-Client -> ZeroDDS Bench ---"; "$WORK/omni_client" bench "$(ior_of BENCH_IOR "$WORK/z.ior")" && \
    RESULTS+=("ZeroDDS-Server<-omniORB: OK") || { RESULTS+=("ZeroDDS-Server<-omniORB: FAIL"); RC=1; }
  kill %1 2>/dev/null; wait %1 2>/dev/null
else echo "omniORB not available — skipped"; RESULTS+=("omniORB: SKIP"); fi

echo
echo "############## TAO ##############"
if build_tao; then
  "$WORK/tao_server" > "$WORK/t.ior" 2>/dev/null & wait_ior "$WORK/t.ior"
  zerodds_calls "TAO" "$WORK/t.ior" && RESULTS+=("TAO-Server<-ZeroDDS: OK") || { RESULTS+=("TAO-Server<-ZeroDDS: FAIL"); RC=1; }
  kill %1 2>/dev/null; wait %1 2>/dev/null
  "$ZSRV" 127.0.0.1 0 > "$WORK/z.ior" 2>/dev/null & wait_ior "$WORK/z.ior"
  echo "--- TAO-Client -> ZeroDDS Echo ---";  "$WORK/tao_client" echo  "$(ior_of ECHO_IOR  "$WORK/z.ior")" && \
  echo "--- TAO-Client -> ZeroDDS Bench ---"; "$WORK/tao_client" bench "$(ior_of BENCH_IOR "$WORK/z.ior")" && \
    RESULTS+=("ZeroDDS-Server<-TAO: OK") || { RESULTS+=("ZeroDDS-Server<-TAO: FAIL"); RC=1; }
  kill %1 2>/dev/null; wait %1 2>/dev/null
else echo "TAO not available — skipped"; RESULTS+=("TAO: SKIP"); fi

echo
echo "############## JacORB ##############"
if build_jacorb; then
  jacorb_server > "$WORK/j.ior" 2>/dev/null & wait_ior "$WORK/j.ior"
  zerodds_calls "JacORB" "$WORK/j.ior" && RESULTS+=("JacORB-Server<-ZeroDDS: OK") || { RESULTS+=("JacORB-Server<-ZeroDDS: FAIL"); RC=1; }
  kill %1 2>/dev/null; wait %1 2>/dev/null
  "$ZSRV" 127.0.0.1 0 > "$WORK/z.ior" 2>/dev/null & wait_ior "$WORK/z.ior"
  echo "--- JacORB-Client -> ZeroDDS Echo ---";  jacorb_client echo  "$(ior_of ECHO_IOR  "$WORK/z.ior")" && \
  echo "--- JacORB-Client -> ZeroDDS Bench ---"; jacorb_client bench "$(ior_of BENCH_IOR "$WORK/z.ior")" && \
    RESULTS+=("ZeroDDS-Server<-JacORB: OK") || { RESULTS+=("ZeroDDS-Server<-JacORB: FAIL"); RC=1; }
  kill %1 2>/dev/null; wait %1 2>/dev/null
else echo "JacORB not available — skipped"; RESULTS+=("JacORB: SKIP"); fi

echo
echo "############## CosNaming (echtes OMG-NamingContext-Wire) ##############"
# Foreign client → ZeroDDS NamingContext server: each ORB narrows our
# NAMING_IOR (type_id IDL:omg.org/CosNaming/NamingContext:1.0) and drives
# bind/resolve/rebind/unbind. ZeroDDS client → foreign daemon: against
# omniNames / JacORB-ns / TAO-tao_cosnaming.
wait_naming_ior() { for _ in $(seq 1 75); do grep -q "^NAMING_IOR=" "$1" 2>/dev/null && return 0; sleep 0.2; done; return 1; }

# --- omniORB ---
if command -v omniidl >/dev/null && command -v omniNames >/dev/null; then
  g++ -O2 -std=c++17 -I"$WORK" "$HERE/omniorb/naming_client.cc" -o "$WORK/omni_naming" \
      -lomniORB4 -lomnithread -lpthread 2>/dev/null && OMNI_N=1 || OMNI_N=0
  # a) omniORB client → ZeroDDS
  "$ZSRV" 127.0.0.1 0 > "$WORK/zn.ior" 2>/dev/null & wait_naming_ior "$WORK/zn.ior"
  if [ "$OMNI_N" = 1 ]; then
    echo "--- omniORB-Client -> ZeroDDS NamingContext ---"
    "$WORK/omni_naming" "$(ior_of NAMING_IOR "$WORK/zn.ior")" \
      && RESULTS+=("ZeroDDS-Naming<-omniORB: OK") || { RESULTS+=("ZeroDDS-Naming<-omniORB: FAIL"); RC=1; }
  fi
  kill %1 2>/dev/null; wait %1 2>/dev/null
  # b) ZeroDDS client → omniNames
  rm -f "$WORK"/omninames-* 2>/dev/null
  # omniNames writes the root context IOR to stdout → capture it.
  omniNames -start 28099 -datadir "$WORK" > "$WORK/omni_ns.log" 2>&1 & sleep 3
  ONIOR=$(grep -aoE "IOR:[0-9a-fA-F]+" "$WORK/omni_ns.log" 2>/dev/null | head -1)
  if [ -n "$ONIOR" ]; then
    echo "--- ZeroDDS-Client -> omniNames ---"
    "$ZCLI" naming "$ONIOR" && RESULTS+=("omniNames<-ZeroDDS: OK") || { RESULTS+=("omniNames<-ZeroDDS: FAIL"); RC=1; }
  else RESULTS+=("omniNames<-ZeroDDS: SKIP (no IOR)"); fi
  kill %1 2>/dev/null; wait %1 2>/dev/null
else RESULTS+=("omniORB-Naming: SKIP"); fi

# --- TAO ---
if [ -x "$TAO_PREFIX/bin/tao_idl" ]; then
  export ACE_ROOT="$TAO_PREFIX/share/ace" TAO_ROOT="$TAO_PREFIX/share/tao" LD_LIBRARY_PATH="$TAO_PREFIX/lib:${LD_LIBRARY_PATH:-}"
  ( cd "$WORK" && "$TAO_PREFIX/bin/tao_idl" -I"$TAO_PREFIX/include" cosnaming.idl ) >/dev/null 2>&1
  g++ -O2 -std=c++17 -I"$WORK" -I"$TAO_PREFIX/include" "$HERE/tao/naming_client.cpp" "$WORK/cosnamingC.cpp" \
      -o "$WORK/tao_naming" -L"$TAO_PREFIX/lib" -lTAO_AnyTypeCode -lTAO -lACE -lpthread 2>/dev/null && TAO_N=1 || TAO_N=0
  # a) TAO client → ZeroDDS
  "$ZSRV" 127.0.0.1 0 > "$WORK/zn.ior" 2>/dev/null & wait_naming_ior "$WORK/zn.ior"
  if [ "$TAO_N" = 1 ]; then
    echo "--- TAO-Client -> ZeroDDS NamingContext ---"
    "$WORK/tao_naming" "$(ior_of NAMING_IOR "$WORK/zn.ior")" \
      && RESULTS+=("ZeroDDS-Naming<-TAO: OK") || { RESULTS+=("ZeroDDS-Naming<-TAO: FAIL"); RC=1; }
  fi
  kill %1 2>/dev/null; wait %1 2>/dev/null
  # b) ZeroDDS client → TAO Naming_Service (tao_cosnaming, if built from orbsvcs)
  TAO_NS="${TAO_NAMING_BIN:-/root/build/acetao/ACE_wrappers/TAO/orbsvcs/Naming_Service/tao_cosnaming}"
  if [ -x "$TAO_NS" ]; then
    LD_LIBRARY_PATH="/opt/opendds-secure/lib:/root/build/acetao/ACE_wrappers/lib" \
      "$TAO_NS" -ORBEndpoint iiop://127.0.0.1:28100 -o "$WORK/tao_ns.ior" >/dev/null 2>&1 & sleep 3
    if [ -s "$WORK/tao_ns.ior" ]; then
      echo "--- ZeroDDS-Client -> TAO Naming_Service ---"
      "$ZCLI" naming "$(cat "$WORK/tao_ns.ior")" && RESULTS+=("TAO-Naming<-ZeroDDS: OK") || { RESULTS+=("TAO-Naming<-ZeroDDS: FAIL"); RC=1; }
    else RESULTS+=("TAO-Naming<-ZeroDDS: SKIP (no IOR)"); fi
    kill %1 2>/dev/null; wait %1 2>/dev/null
  else RESULTS+=("TAO-Naming<-ZeroDDS: SKIP (orbsvcs/tao_cosnaming not built)"); fi
else RESULTS+=("TAO-Naming: SKIP"); fi

# --- JacORB ---
if [ -x "$JAVA" ] && [ -d "$JACORB" ]; then
  "$JAVAC" -encoding UTF-8 -cp "$JCP" -d "$WORK" "$HERE/jacorb/NamingClient.java" 2>/dev/null && JAC_N=1 || JAC_N=0
  # a) JacORB client → ZeroDDS
  "$ZSRV" 127.0.0.1 0 > "$WORK/zn.ior" 2>/dev/null & wait_naming_ior "$WORK/zn.ior"
  if [ "$JAC_N" = 1 ]; then
    echo "--- JacORB-Client -> ZeroDDS NamingContext ---"
    ( cd "$WORK" && "$JAVA" $JPROPS -cp ".:$JCP" NamingClient "$(ior_of NAMING_IOR "$WORK/zn.ior")" ) \
      && RESULTS+=("ZeroDDS-Naming<-JacORB: OK") || { RESULTS+=("ZeroDDS-Naming<-JacORB: FAIL"); RC=1; }
  fi
  kill %1 2>/dev/null; wait %1 2>/dev/null
  # b) ZeroDDS client → JacORB ns
  "$JAVA" -Djava.endorsed.dirs="$JACORB/lib" -Djacorb.home="$JACORB" \
    -Djacorb.naming.ior_filename="$WORK/jac_ns.ior" $JPROPS -cp "$JCP" \
    org.jacorb.naming.NameServer >/dev/null 2>&1 &
  for _ in $(seq 1 80); do [ -s "$WORK/jac_ns.ior" ] && break; sleep 0.1; done
  if [ -s "$WORK/jac_ns.ior" ]; then
    echo "--- ZeroDDS-Client -> JacORB ns ---"
    "$ZCLI" naming "$(cat "$WORK/jac_ns.ior")" && RESULTS+=("JacORB-ns<-ZeroDDS: OK") || { RESULTS+=("JacORB-ns<-ZeroDDS: FAIL"); RC=1; }
  else RESULTS+=("JacORB-ns<-ZeroDDS: SKIP (no IOR)"); fi
  kill %1 2>/dev/null; wait %1 2>/dev/null
else RESULTS+=("JacORB-Naming: SKIP"); fi

# --- SSLIOP (IIOP over TLS) -------------------------------------------------
echo
echo "############## SSLIOP (IIOP over TLS) ##############"
# Self-signed leaf cert (CA:FALSE — webpki/rustls rejects CA:TRUE as an
# end entity; SAN=localhost for SNI verification). The same PEM serves both
# ZeroDDS AND omniORB as identity and mutual root CA.
SSL_CERT="$WORK/ssl_cert.pem"; SSL_KEY="$WORK/ssl_key.pem"
openssl req -x509 -newkey rsa:2048 -keyout "$SSL_KEY" -out "$SSL_CERT" -days 1 -nodes \
  -subj "/CN=localhost" \
  -addext "subjectAltName=DNS:localhost" \
  -addext "basicConstraints=critical,CA:FALSE" \
  -addext "keyUsage=critical,digitalSignature" \
  -addext "extendedKeyUsage=serverAuth" >/dev/null 2>&1
# omniORB has no separate certificate_file — key_file carries cert + key in
# ONE PEM (cert FIRST: omniORB reads the first PEM object as the certificate).
# ZeroDDS (rustls) uses cert/key separately.
SSL_KEYCERT="$WORK/ssl_keycert.pem"; cat "$SSL_CERT" "$SSL_KEY" > "$SSL_KEYCERT"

# a) ZeroDDS <-> ZeroDDS SSLIOP baseline (cross-process, real TLS).
"$ZSSLSRV" "$SSL_CERT" "$SSL_KEY" > "$WORK/zssl.ior" 2>/dev/null &
for _ in $(seq 1 75); do grep -q "^SSLIOP_IOR=" "$WORK/zssl.ior" 2>/dev/null && break; sleep 0.2; done
if grep -q "^SSLIOP_IOR=" "$WORK/zssl.ior" 2>/dev/null; then
  echo "--- ZeroDDS-Client -> ZeroDDS SSLIOP ---"
  "$ZSSLCLI" "$(ior_of SSLIOP_IOR "$WORK/zssl.ior")" "$SSL_CERT" \
    && RESULTS+=("ZeroDDS-SSLIOP<-ZeroDDS: OK") || { RESULTS+=("ZeroDDS-SSLIOP<-ZeroDDS: FAIL"); RC=1; }
else RESULTS+=("ZeroDDS-SSLIOP<-ZeroDDS: SKIP (no IOR)"); fi
kill %1 2>/dev/null; wait %1 2>/dev/null

# omniORB SSLIOP — only if the SSL transport (libomnisslTP4) is available.
if command -v omniidl >/dev/null && ldconfig -p 2>/dev/null | grep -q omnisslTP; then
  OSSL_CF="-O2 -std=c++17 -I$WORK -DOMNI_UNLOADABLE_STUBS"
  OSSL_LB="-lomnisslTP4 -lomniORB4 -lomnithread -lomniDynamic4 -lssl -lcrypto -lpthread"
  ( cd "$WORK" && omniidl -bcxx -Wba interop.idl ) >/dev/null 2>&1
  OSSL_OK=1
  g++ $OSSL_CF "$HERE/omniorb/ssliop_client.cc" "$WORK/interopSK.cc" "$WORK/interopDynSK.cc" -o "$WORK/omni_ssl_client" $OSSL_LB 2>/dev/null || OSSL_OK=0
  g++ $OSSL_CF "$HERE/omniorb/ssliop_server.cc" "$WORK/interopSK.cc" "$WORK/interopDynSK.cc" -o "$WORK/omni_ssl_server" $OSSL_LB 2>/dev/null || OSSL_OK=0
  if [ "$OSSL_OK" = 1 ]; then
    # b) omniORB client -> ZeroDDS SSLIOP server.
    "$ZSSLSRV" "$SSL_CERT" "$SSL_KEY" > "$WORK/zssl2.ior" 2>/dev/null &
    for _ in $(seq 1 75); do grep -q "^SSLIOP_IOR=" "$WORK/zssl2.ior" 2>/dev/null && break; sleep 0.2; done
    if grep -q "^SSLIOP_IOR=" "$WORK/zssl2.ior" 2>/dev/null; then
      echo "--- omniORB-Client -> ZeroDDS SSLIOP ---"
      "$WORK/omni_ssl_client" "$(ior_of SSLIOP_IOR "$WORK/zssl2.ior")" "$SSL_CERT" "$SSL_KEYCERT" \
        && RESULTS+=("ZeroDDS-SSLIOP<-omniORB: OK") || { RESULTS+=("ZeroDDS-SSLIOP<-omniORB: FAIL"); RC=1; }
    else RESULTS+=("ZeroDDS-SSLIOP<-omniORB: SKIP (no IOR)"); fi
    kill %1 2>/dev/null; wait %1 2>/dev/null
    # c) ZeroDDS client -> omniORB SSLIOP server.
    "$WORK/omni_ssl_server" "$SSL_CERT" "$SSL_KEYCERT" > "$WORK/ossl.ior" 2>/dev/null &
    for _ in $(seq 1 75); do grep -q "^SSLIOP_IOR=" "$WORK/ossl.ior" 2>/dev/null && break; sleep 0.2; done
    if grep -q "^SSLIOP_IOR=" "$WORK/ossl.ior" 2>/dev/null; then
      echo "--- ZeroDDS-Client -> omniORB SSLIOP ---"
      "$ZSSLCLI" "$(ior_of SSLIOP_IOR "$WORK/ossl.ior")" "$SSL_CERT" \
        && RESULTS+=("omniORB-SSLIOP<-ZeroDDS: OK") || { RESULTS+=("omniORB-SSLIOP<-ZeroDDS: FAIL"); RC=1; }
    else RESULTS+=("omniORB-SSLIOP<-ZeroDDS: SKIP (no IOR)"); fi
    kill %1 2>/dev/null; wait %1 2>/dev/null
  else RESULTS+=("omniORB-SSLIOP: SKIP (Build fehlgeschlagen)"); fi
else RESULTS+=("omniORB-SSLIOP: SKIP (libomnisslTP not available)"); fi
# TAO SSLIOP: /opt/opendds-secure was built without libTAO_SSLIOP → no TAO SSL.
RESULTS+=("TAO-SSLIOP: SKIP (built without TAO_SSLIOP)")

echo
echo "================ ZUSAMMENFASSUNG ================"
for r in "${RESULTS[@]}"; do echo "  $r"; done
echo "================================================"
[ "$RC" -eq 0 ] && echo "CROSS-ORB INTEROP: all available combinations GREEN" || echo "CROSS-ORB INTEROP: ERROR (RC=$RC)"
exit $RC
