#!/usr/bin/env bash
# Builds the gRPC clients of crates/grpc-bridge/tests/cross_vendor.rs
# (zerodds-grpc-bridge-1.0 §10, §12.3) from the .proto the crate ships
# (crates/grpc-bridge/proto/zerodds/chat/v1/bridge.proto), before the tests
# run, so that compiling them is not counted against a test's 25 seconds:
#
#   grpc-go-client     grpc-go: the stubs of protoc-gen-go and
#                      protoc-gen-go-grpc, built in GOPATH mode against
#                      Debian's packages (golang-google-grpc-dev), offline
#   grpc-java-client   grpc-java: the stubs of protoc's grpc-java plugin,
#                      compiled against Debian's libgrpc-java 1.41 (Netty
#                      transport); a launcher script
#   tonic-client       tonic: a cargo project of its own with its lock file
#                      (tests/interop/tonic), stubs by tonic-prost-build
#
#     bash crates/grpc-bridge/tests/interop/build_peers.sh <dir>
#
# The tests find the programs under $ZERODDS_IT_PEERS/grpc, which is the
# <dir> the integration pillar passes ({peers}/grpc, ci/integration_scenarios.py).
# What the programs need: tests/integration/provision.sh grpc-go grpc-java
# protoc, and cargo with crates.io for the first build of tonic-client.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
proto="$(cd "$here/../../proto" && pwd)"
out="${1:?usage: build_peers.sh <dir>}"
mkdir -p "$out"
out="$(cd "$out" && pwd)"
file=zerodds/chat/v1/bridge.proto
failed=()

build_go() {
  local gopath="$out/go" package=zerodds.org/grpcpeer
  rm -rf "$gopath/src/$package" && mkdir -p "$gopath/src/$package/client"
  protoc -I "$proto" \
    --go_out="$gopath/src" --go_opt="M$file=$package/chatv1" \
    --go-grpc_out="$gopath/src" --go-grpc_opt="M$file=$package/chatv1" \
    "$file" || return 1
  cp "$here/go/main.go" "$gopath/src/$package/client/main.go" || return 1
  # GOPATH mode: the modules are Debian's sources under /usr/share/gocode;
  # nothing is downloaded, no toolchain either.
  GOPATH="$gopath:/usr/share/gocode" GO111MODULE=off GOFLAGS= GOPROXY=off GOTOOLCHAIN=local \
    GOCACHE="$out/go-cache" go build -o "$out/grpc-go-client" "$package/client"
}

build_java() {
  local src="$out/java-src" classes="$out/java-classes"
  # The jars name the rest of grpc-java's dependencies (Netty, Guava,
  # perfmark, gson) in their Class-Path manifests, but for grpc-protobuf-lite,
  # which grpc-protobuf calls (ProtoLiteUtils), and netty-resolver, which
  # grpc-netty calls (AddressResolverGroup). The plugin of grpc-java
  # 1.41 annotates the stub with javax.annotation.Generated, which the JDK
  # no longer carries: Debian's libgeronimo-annotation-1.3-spec-java has it.
  local cp=/usr/share/java/grpc-netty.jar:/usr/share/java/grpc-protobuf.jar:/usr/share/java/grpc-protobuf-lite.jar
  cp="$cp:/usr/share/java/grpc-stub.jar:/usr/share/java/netty-resolver.jar"
  cp="$cp:/usr/share/java/protobuf.jar:/usr/share/java/geronimo-annotation-1.3-spec.jar"
  rm -rf "$src" "$classes" && mkdir -p "$src" "$classes"
  protoc -I "$proto" --java_out="$src" \
    --plugin=protoc-gen-grpc-java="$(command -v grpc_java_plugin)" \
    --grpc-java_out="$src" "$file" || return 1
  javac -nowarn -d "$classes" -cp "$cp" $(find "$src" -name '*.java') "$here/java/BridgeClient.java" \
    || return 1
  cat > "$out/grpc-java-client" <<SH
#!/bin/sh
exec java -cp "$classes:$cp" BridgeClient "\$@"
SH
  chmod +x "$out/grpc-java-client"
}

build_tonic() {
  # A target directory of its own: the project is no workspace member.
  CARGO_TARGET_DIR="$out/tonic-target" cargo build -q --release --locked \
    --manifest-path "$here/tonic/Cargo.toml" || return 1
  cp "$out/tonic-target/release/tonic-client" "$out/tonic-client"
}

for peer in go java tonic; do
  started=$SECONDS
  if "build_$peer"; then
    echo "build_peers: $peer in $((SECONDS - started)) s"
  else
    failed+=("$peer")
  fi
done
echo "build_peers: $(ls "$out" | tr '\n' ' ')"
# `set -e` does not hold inside a function called as an `if` condition:
# each step above ends its function with `|| return 1`.
# Each client is built even when another one fails, so that one broken
# toolchain does not take the other tests with it; the job fails anyway.
if [ "${#failed[@]}" -gt 0 ]; then
  echo "build_peers: failed: ${failed[*]}" >&2
  exit 1
fi
