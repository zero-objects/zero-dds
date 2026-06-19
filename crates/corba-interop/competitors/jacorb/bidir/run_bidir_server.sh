#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Live bidirectional-GIOP cross-ORB (#21): builds + starts the JacORB bidir server
# (demo/bidir, BiDirPOA) and prints its IOR on stdout (BIDIR_SERVER_IOR=). The
# ZeroDDS test `bidir_cross_orb::jacorb_server_callbacks_zerodds_over_shared_connection`
# opens a BiDir connection as originator, registers a ZeroDDS callback and
# calls callback_hello → the JacORB server calls hello() BACK over the SAME connection.
#
# The Linux test host (JDK8, JacORB /opt/jacorb). Verified 2026-06-07:
#   "Server object received hello message >Hi from ZeroDDS<" + test green.
# Important: OAIAddr/ipAddr=127.0.0.1 forces IPv4 so that JacORB's BiDir connection
# reuse correlation matches the client's IPv4 listen point (otherwise IPv6 drift).
set -euo pipefail
JAVA=/opt/jdk8/bin/java; JAVAC=/opt/jdk8/bin/javac; JACORB=/opt/jacorb
JPROPS="-Dorg.omg.CORBA.ORBClass=org.jacorb.orb.ORB -Dorg.omg.CORBA.ORBSingletonClass=org.jacorb.orb.ORBSingleton"
BIDIRP="-Dorg.omg.PortableInterceptor.ORBInitializerClass.bidir_init=org.jacorb.orb.giop.BiDirConnectionInitializer \
        -Dorg.jacorb.ipAddr=127.0.0.1 -DOAIAddr=127.0.0.1"
D=/opt/jacorb/demo/bidir/src/main
W="$(mktemp -d)"; mkdir -p "$W/build" "$W/gen"; cd "$W"
java -classpath "$JACORB/lib/idl.jar" org.jacorb.idl.parser -d gen "$D/idl"/*.idl
CP="$(ls "$JACORB"/lib/*.jar | tr '\n' ':')"
"$JAVAC" -encoding UTF-8 -cp "$CP" -d build $(find gen -name '*.java') \
    "$D/java/org/jacorb/demo/bidir/ServerImpl.java"
# shellcheck disable=SC2086
"$JAVA" $JPROPS $BIDIRP -cp "build:$CP" org.jacorb.demo.bidir.ServerImpl "$W/b.ior" &
SRV=$!
for _ in $(seq 1 40); do [ -s "$W/b.ior" ] && break; sleep 0.5; done
echo "BIDIR_SERVER_IOR=$(head -1 "$W/b.ior")"
wait $SRV
