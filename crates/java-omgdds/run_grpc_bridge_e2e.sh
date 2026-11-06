#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Java↔Rust multi-process e2e for the gRPC DDS bridge (F2b): the Maven
# `GrpcBridgeClientTest` (pure-Java gRPC client) against a running
# `zerodds-grpc-bridged --features dds-runtime`.
#
# tests/integration/with_domains.py leases the DDS domain,
# tests/integration/with_grpc_bridge.py builds and starts the daemon on it
# (topic Demo::Bytes as DemoStream, TRANSIENT_LOCAL), waits for its
# `listening` event and passes the port as ZERODDS_BRIDGE_PORT. The Java test
# publishes via gRPC → DDS DataWriter, then subscribes: the stream's own
# DataReader receives the sample as a late joiner. The weekly job
# `integration-java-psm` (ci/integration_scenarios.py) runs the same wrappers
# around the test's JUnit tag `bridge-daemon`.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"
python3 tests/integration/with_domains.py 1 -- \
    python3 tests/integration/with_grpc_bridge.py \
    bash -c 'cd crates/java-omgdds/java && mvn -q -Dgroups=bridge-daemon test'
