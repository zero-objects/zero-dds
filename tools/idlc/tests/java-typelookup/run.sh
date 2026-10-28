#!/usr/bin/env bash
# Canonical Java8 packaged-artifact TypeLookup gate. No vendor installation or
# checkout production class directory is used by the counterprocess consumers.
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../../.." && pwd)
cd "$repo"
artifacts=${1:-"$repo/target/java-typelookup-gate"}
# Domain42 is reserved by zerodds-test-domains and stays below ephemeral ports.
# CI serializes this job via dcps-multicast. Standalone callers must ensure
# exclusive use of their explicitly selected domain (or the default42).
domain=${2:-42}
case "$domain" in ''|*[!0-9]*) echo "Invalid DDS domain: $domain" >&2; exit 2;; esac
if (( domain > 231 )); then echo "DDS domain exceeds UDP port range" >&2; exit 2; fi
mkdir -p "$artifacts"
artifacts=$(cd "$artifacts" && pwd)
run=$(mktemp -d "$artifacts/run.XXXXXX")
exec > >(tee "$run/gate.log") 2>&1
cleanup() {
    status=$?
    if test -n "${server_pid:-}"; then
        kill "$server_pid" 2>/dev/null || true
        wait "$server_pid" 2>/dev/null || true
    fi
    printf 'JAVA_TYPELOOKUP_GATE_EXIT=%s\n' "$status"
    printf 'JAVA_TYPELOOKUP_GATE_RUN=%s\n' "$run"
}
trap cleanup EXIT
mkdir "$run/fixture" "$run/consumer"
cargo metadata --locked --no-deps --format-version 1 > "$run/cargo-metadata.json"
version=$(python3 -c 'import json,sys; m=json.load(open(sys.argv[1])); print(next(p["version"] for p in m["packages"] if p["name"] == "zerodds-idlc"))' "$run/cargo-metadata.json")
cargo_target=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["target_directory"])' "$run/cargo-metadata.json")
cargo build -p zerodds-idlc --locked --jobs 1
"$cargo_target/debug/zerodds-idlc" generate --lang java --java-mapping omg --encoding xcdr2 \
    --out "$run/fixture" crates/java-omgdds/java/src/test/idl/typelookup-pagination.idl
# Keep the existing binding tests required, now on the real Java8 API surface.
MAVEN_OPTS="${MAVEN_OPTS:-} -Xmx768m" mvn -B -f crates/java-omgdds/java/pom.xml -Pjava8 -DargLine=-Xmx256m clean verify
artifact="$repo/crates/java-omgdds/java/target/omgdds-$version.jar"
test -s "$artifact"
jar tf "$artifact" > "$run/jar-entries.txt"
grep -Fx 'org/zerodds/wire/Reader.class' "$run/jar-entries.txt"
grep -Fx 'META-INF/typelookup/LICENSE.cyclone' "$run/jar-entries.txt"
grep -Fx 'META-INF/typelookup/NOTICE.cyclone.md' "$run/jar-entries.txt"
javap -verbose -classpath "$artifact" org.zerodds.typelookup.TypeLookupService > "$run/class-version.txt"
grep -F 'major version: 52' "$run/class-version.txt"
javac -J-Xmx384m --release 8 -cp "$artifact" -d "$run/consumer" \
    "$run/fixture/"*.java tools/idlc/tests/java-typelookup/ReliableWindowCheck.java
javac -J-Xmx256m --release 17 -cp "$artifact:$run/consumer" -d "$run/consumer" \
    tools/idlc/tests/java-typelookup/TypeLookupProcess.java \
    tools/idlc/tests/java-typelookup/ExportTypeObjects.java
classpath="$run/consumer:$artifact"
java -Xmx128m -cp "$classpath" org.zerodds.rtps.ReliableWindowCheck
test "$(wc -l < crates/types/tests/fixtures/cyclone-typelookup-11.log)" -eq 4
java -Xmx256m -cp "$classpath" TypeLookupProcess oracle crates/types/tests/fixtures/cyclone-typelookup-11.log
java -Xmx256m -cp "$classpath" TypeLookupProcess server "$domain" "$run/server-prefix" > "$run/server.log" 2>&1 &
server_pid=$!
for attempt in $(seq 1 50); do
    test -s "$run/server-prefix" && break
    kill -0 "$server_pid"
    sleep 0.1
done
test -s "$run/server-prefix"
prefix=$(head -1 "$run/server-prefix")
port=$(tail -1 "$run/server-prefix")
java -Xmx256m -cp "$classpath" TypeLookupProcess client "$domain" "$prefix" | tee "$run/client.log"
java -Xmx256m -cp "$classpath" TypeLookupProcess loss "$domain" "$prefix" "$port" | tee "$run/loss.log"
grep -Fx 'JAVA_TYPELOOKUP_LIVE=ROOT_EXACT,DEPENDENCIES_65,PAGES_2,UNKNOWN_EMPTY' "$run/client.log"
grep -Fx 'JAVA_TYPELOOKUP_LOSS=FRAGMENT_DROPPED_1,NACK_FRAG_REPAIRED_1,ROOT_EXACT' "$run/loss.log"
# The Java server has completed its checks; do not keep its heap alongside the
# native compilation. Only our own child PID is stopped.
if kill -0 "$server_pid" 2>/dev/null; then kill "$server_pid"; fi
wait "$server_pid" 2>/dev/null || true
unset server_pid
# Reuse the exact generated closure and packaged peer for the native transport.
# The otherwise ignored external-process test is mandatory here, including an
# assertion that exactly one test actually executed (a wrong filter is not green).
java -Xmx256m -cp "$classpath" ExportTypeObjects "$run/native-typeobjects"
# DCPS dev-dependencies include recorder/security packages not built by IDLC.
# A fresh runner may fetch those locked dependencies; cache warmth is not a
# test prerequisite. Assertions and the exact one-test requirement stay strict.
TYPELOOKUP_FIXTURE_DIR="$run/native-typeobjects" TYPELOOKUP_JAVA_CLASSPATH="$classpath" \
    cargo test -p zerodds-dcps --locked --jobs 1 --test type_lookup_transport_live \
    native_large_reply_repairs_lost_fragment -- --include-ignored --exact --test-threads=1 --nocapture \
    | tee "$run/native-loss.log"
grep -F 'test result: ok. 1 passed; 0 failed; 0 ignored;' "$run/native-loss.log"
