#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Cross-ORB valuetype wire capture against JacORB 3.9 (#1 §15.3.4).
# Runs on the Linux test host (Debian 13, JacORB under /opt/jacorb). Produces the
# golden vector (JacORB encode) and checks the decode reverse direction with ZeroDDS bytes.
#
# Expectation (Point(42,-7), big-endian):
#   7fffff020000000e49444c3a506f696e743a312e300000000000002afffffff9
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
JACORB_HOME="${JACORB_HOME:-/opt/jacorb}"
W="$(mktemp -d)"; trap 'rm -rf "$W"' EXIT
cp "$HERE"/Point.idl "$HERE"/PointImpl.java "$HERE"/Dumper.java "$W/"
cd "$W"

# Generate stubs (Point.java/PointHelper.java/PointHolder.java).
java -classpath "$JACORB_HOME/lib/idl.jar" org.jacorb.idl.parser -d gen Point.idl
cp PointImpl.java Dumper.java gen/

CP="$(ls "$JACORB_HOME"/lib/*.jar | tr '\n' ':')"
javac -cp "$CP" -d build gen/*.java

# ZeroDDS reference bytes (from value_wire::tests::jacorb_capture_byte_identical).
ZBYTES="7fffff020000000e49444c3a506f696e743a312e300000000000002afffffff9"
java -cp "build:$CP" Dumper "$ZBYTES" 2>/dev/null | grep JACORB
