#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Compiles + runs the JacORB transactional OTS client against a ZeroDDS IOR.
# Arg 1 = stringified IOR of the ZeroDDS target (from the Rust handshake test).
#
# Prerequisite (codepit): a JacORB NameServer + TransactionService must be
# running; their setup is below. The client resolves the OTS `Current` from the
# running TransactionService and `begin()`s a transaction, so JacORB's
# ClientContextTransferInterceptor attaches the OTS PropagationContext as IIOP
# service context id=0.
#
#   CP="$(ls /opt/jacorb/lib/*.jar | tr '\n' ':')"
#   JP="-Dorg.omg.CORBA.ORBClass=org.jacorb.orb.ORB -Dorg.omg.CORBA.ORBSingletonClass=org.jacorb.orb.ORBSingleton"
#   # NameServer (writes its IOR):
#   /opt/jdk8/bin/java $JP -Djacorb.naming.ior_filename=/tmp/otslive/ns.ior \
#       -cp "$CP" org.jacorb.naming.NameServer &
#   # TransactionService (registers itself in the NameService):
#   /opt/jdk8/bin/java $JP -cp "$CP" org.jacorb.transaction.TransactionService \
#       -ORBInitRef NameService=file:///tmp/otslive/ns.ior &
#   export NS_IOR=/tmp/otslive/ns.ior
set -euo pipefail
IOR="${1:?usage: run_ots_client.sh <IOR>}"

JAVA="${JDK8:-/opt/jdk8}/bin/java"
JAVAC="${JDK8:-/opt/jdk8}/bin/javac"
JACORB="${JACORB_HOME:-/opt/jacorb}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
NS_IOR="${NS_IOR:-/tmp/otslive/ns.ior}"

JPROPS="-Dorg.omg.CORBA.ORBClass=org.jacorb.orb.ORB \
        -Dorg.omg.CORBA.ORBSingletonClass=org.jacorb.orb.ORBSingleton \
        -Dorg.omg.PortableInterceptor.ORBInitializerClass.transaction=org.jacorb.transaction.TransactionInitializer"

CP="$(ls "$JACORB"/lib/*.jar | tr '\n' ':')"
W="$(mktemp -d)"
trap 'rm -rf "$W"' EXIT
"$JAVAC" -encoding UTF-8 -cp "$CP" -d "$W" "$HERE/OtsClient.java"
# shellcheck disable=SC2086
"$JAVA" $JPROPS -cp "$W:$CP" OtsClient "$IOR" -ORBInitRef NameService="file://$NS_IOR"
