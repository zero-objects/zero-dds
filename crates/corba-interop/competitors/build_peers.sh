#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Builds the foreign ORB programs the cross-ORB tests of zerodds-corba-interop
# start (tests/omniorb_cross_orb.rs, tests/tao_cross_orb.rs,
# tests/jacorb_cross_orb.rs), each from the IDL ZeroDDS generates its side
# from (interop.idl, cosnaming.idl, features.idl), with that ORB's own IDL
# compiler. The tests take them from the directory named by
# ZERODDS_CORBA_PEERS and fail when one is missing.
#
#     bash crates/corba-interop/competitors/build_peers.sh <omniorb|tao|jacorb> <outdir>
#
# omniORB: omniidl and g++ (Debian's omniorb, omniidl, libomniorb4-dev; the
# SSLIOP programs link libomnisslTP4 and OpenSSL). TAO: tao_idl and the TAO
# libraries under TAO_PREFIX (default /opt/opendds, the TAO of the OpenDDS
# build in the interop image). JacORB: JacORB's IDL compiler and javac of the
# Java 8 JDK, under JACORB_HOME (/opt/jacorb) and JACORB_JAVA_HOME (/opt/jdk8).
set -euo pipefail
orb="${1:?usage: build_peers.sh <omniorb|tao|jacorb> <outdir>}"
out="${2:?usage: build_peers.sh <omniorb|tao|jacorb> <outdir>}"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
mkdir -p "$out"
out="$(cd "$out" && pwd)"
work="$out/$orb-gen"
rm -rf "$work"
mkdir -p "$work"
cp "$here/interop.idl" "$here/cosnaming.idl" "$here/features.idl" "$work/"

case "$orb" in
  omniorb)
    ( cd "$work" && omniidl -bcxx -Wba interop.idl && omniidl -bcxx -Wba features.idl )
    cf=(-O1 -std=c++17 -I"$work" -DOMNI_UNLOADABLE_STUBS)
    lb=(-lomniORB4 -lomnithread -lomniDynamic4 -lpthread)
    ssl=(-lomnisslTP4 -lomniORB4 -lomnithread -lomniDynamic4 -lssl -lcrypto -lpthread)
    pids=()
    for p in interop_server interop_client; do
      g++ "${cf[@]}" "$here/omniorb/$p.cc" "$work/interopSK.cc" "$work/interopDynSK.cc" \
          -o "$out/omniorb_$p" "${lb[@]}" & pids+=($!)
    done
    for p in ssliop_server ssliop_client; do
      g++ "${cf[@]}" "$here/omniorb/$p.cc" "$work/interopSK.cc" "$work/interopDynSK.cc" \
          -o "$out/omniorb_$p" "${ssl[@]}" & pids+=($!)
    done
    for p in features_server features_client; do
      g++ "${cf[@]}" "$here/omniorb/$p.cc" "$work/featuresSK.cc" "$work/featuresDynSK.cc" \
          -o "$out/omniorb_$p" "${lb[@]}" & pids+=($!)
    done
    g++ "${cf[@]}" "$here/omniorb/naming_client.cc" -o "$out/omniorb_naming_client" \
        -lomniORB4 -lomnithread -lpthread & pids+=($!)
    for pid in "${pids[@]}"; do wait "$pid"; done
    ;;
  tao)
    prefix="${TAO_PREFIX:-/opt/opendds}"
    export ACE_ROOT="$prefix/share/ace" TAO_ROOT="$prefix/share/tao"
    export LD_LIBRARY_PATH="$prefix/lib:${LD_LIBRARY_PATH:-}"
    ( cd "$work" && "$prefix/bin/tao_idl" -I"$prefix/include" -I"$prefix/share/tao" interop.idl \
        && "$prefix/bin/tao_idl" -I"$prefix/include" -I"$prefix/share/tao" cosnaming.idl )
    cf=(-O1 -std=c++17 -I"$work" -I"$prefix/include")
    lb=(-L"$prefix/lib" -Wl,-rpath,"$prefix/lib" -lTAO_PortableServer -lTAO_AnyTypeCode -lTAO -lACE -lpthread)
    pids=()
    for p in interop_server interop_client; do
      g++ "${cf[@]}" "$here/tao/$p.cpp" "$work/interopC.cpp" "$work/interopS.cpp" \
          -o "$out/tao_$p" "${lb[@]}" & pids+=($!)
    done
    g++ "${cf[@]}" "$here/tao/naming_client.cpp" "$work/cosnamingC.cpp" -o "$out/tao_naming_client" \
        -L"$prefix/lib" -Wl,-rpath,"$prefix/lib" -lTAO_AnyTypeCode -lTAO -lACE -lpthread & pids+=($!)
    for pid in "${pids[@]}"; do wait "$pid"; done
    ;;
  jacorb)
    home="${JACORB_HOME:-/opt/jacorb}"
    java_home="${JACORB_JAVA_HOME:-/opt/jdk8}"
    classes="$out/jacorb-classes"
    rm -rf "$classes"
    mkdir -p "$classes"
    "$java_home/bin/java" -cp "$home/lib/idl.jar:$home/lib/antlr-2.7.2.jar" org.jacorb.idl.parser \
        -d "$work/java" "$work/interop.idl"
    # JDK 8's javac reads the platform encoding (ASCII) otherwise.
    "$java_home/bin/javac" -nowarn -encoding UTF-8 -endorseddirs "$home/lib" -cp "$home/lib/*" \
        -d "$classes" $(find "$work/java" -name '*.java') \
        "$here/jacorb/InteropServer.java" "$here/jacorb/InteropClient.java" \
        "$here/jacorb/NamingClient.java" "$here/jacorb/ots/OtsClient.java"
    ;;
  *)
    echo "build_peers: unknown ORB '$orb' (omniorb, tao, jacorb)" >&2
    exit 2
    ;;
esac
echo "build_peers: $orb programs in $out"
# For the steps after this one (ci/run_integration.py reads `ENV` lines).
echo "ENV ZERODDS_CORBA_PEERS=$out"
