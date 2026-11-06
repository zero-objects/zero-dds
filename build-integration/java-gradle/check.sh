#!/usr/bin/env bash
# Builds the Gradle plugin of build-integration/java-gradle and applies it in
# its sample consumer: zerodds-idlc from this checkout, the Java PSM
# (org.zerodds:omgdds) installed into the local Maven repository, then
# `gradle run` of the sample, whose Main holds the octets of the generated
# type support to DDS-XTypes 1.3 §7.4. The job merge-java-gradle of
# ci/integration_scenarios.py runs it.
#
#     bash build-integration/java-gradle/check.sh
#
# Needs a JDK, Maven and Gradle (tests/integration/provision.sh jdk maven gradle).
set -euo pipefail
repo=$(cd "$(dirname "$0")/../.." && pwd)
cd "$repo"
cargo build --locked -q -p zerodds-idlc
target=$(cargo metadata --locked --no-deps --format-version 1 \
    | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
export PATH="$target/debug:$PATH"
mvn -B --no-transfer-progress -q -f crates/java-omgdds/java/pom.xml -DskipTests install
gradle --no-daemon --console=plain -p build-integration/java-gradle/sample-consumer run
