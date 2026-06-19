#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Live CSIv2-SAS handshake cross-ORB (#20): builds + starts the JacORB GssUpServer
# (demo/sas, SecurePOA with EstablishTrustInClient + ListGssUpContext, user jay/test)
# and prints its IOR on stdout (line SECURE_IOR=). The ZeroDDS test
# `csiv2_cross_orb::zerodds_gssup_accepted_by_jacorb_tss` runs against it.
#
# Runs on the Linux test host (JDK8, JacORB /opt/jacorb). Verified 2026-06-07:
#   Server log: "---------> jay, test" + "printSAS for user jay" (accepted),
#               "---------> jay, wrong" + NO_PERMISSION (rejected).
set -euo pipefail
JAVA=/opt/jdk8/bin/java; JAVAC=/opt/jdk8/bin/javac; JACORB=/opt/jacorb
JPROPS="-Dorg.omg.CORBA.ORBClass=org.jacorb.orb.ORB -Dorg.omg.CORBA.ORBSingletonClass=org.jacorb.orb.ORBSingleton"
SASP="-Djacorb.security.sas.contextClass=org.jacorb.demo.sas.ListGssUpContext \
      -Dorg.omg.PortableInterceptor.ORBInitializerClass.SAS=org.jacorb.security.sas.SASInitializer \
      -Djacorb.implname=StandardImplName"
D=/opt/jacorb/demo/sas/src/main
W="$(mktemp -d)"; mkdir -p "$W/build" "$W/gen"; cd "$W"
java -classpath "$JACORB/lib/idl.jar" org.jacorb.idl.parser -d gen "$D/idl/server.idl"
CP="$(ls "$JACORB"/lib/*.jar | tr '\n' ':')"
"$JAVAC" -encoding UTF-8 -cp "$CP" -d build $(find gen -name '*.java') \
    "$D/java/org/jacorb/demo/sas/GssUpServer.java" \
    "$D/java/org/jacorb/demo/sas/ListGssUpContext.java"
# shellcheck disable=SC2086
"$JAVA" $JPROPS $SASP -cp "build:$CP" org.jacorb.demo.sas.GssUpServer "$W/secure.ior" &
SRV=$!
for _ in $(seq 1 40); do [ -s "$W/secure.ior" ] && break; sleep 0.5; done
echo "SECURE_IOR=$(head -1 "$W/secure.ior")"
echo "server PID=$SRV (kill when done)"
wait $SRV
