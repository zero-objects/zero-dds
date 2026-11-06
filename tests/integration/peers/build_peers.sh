#!/usr/bin/env bash
# Builds the foreign client programs of the bridge scenarios before they run,
# so that compiling them is not counted against a scenario's 25 seconds:
#
#   open62541_client  the OPC UA client of crates/opcua-bridge/tests/foreign,
#                     against libopen62541's public C API (pkg-config)
#   calc_client       the omniORB programs of crates/corba-dds-bridge/tests/
#   listener_server   omniorb, from Calc.idl through omniidl and g++
#
#     bash tests/integration/peers/build_peers.sh <dir>
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
out="${1:?usage: build_peers.sh <dir>}"
mkdir -p "$out"

failed=()

# The headers from pkg-config, the library by name: Debian's open62541.pc
# (libopen62541-1.4-dev) lists its private libraries as `-l/usr/lib/...so`,
# which ld cannot resolve (measured in job 227792). libopen62541.so carries
# its own dependencies.
cc "$repo/crates/opcua-bridge/tests/foreign/open62541_client.c" \
  $(pkg-config --cflags open62541) -lopen62541 -o "$out/open62541_client" \
  || failed+=(open62541_client)

omniorb="$repo/crates/corba-dds-bridge/tests/omniorb"
if omniidl -bcxx -C "$out" "$omniorb/Calc.idl"; then
  for program in calc_client listener_server; do
    g++ -std=c++17 -I "$out" "$omniorb/$program.cc" "$out/CalcSK.cc" \
      -lomniORB4 -lomnithread -o "$out/$program" || failed+=("$program")
  done
else
  failed+=(Calc.idl)
fi
echo "build_peers: $(ls "$out" | tr '\n' ' ')"
# Each peer is built even when another one fails, so that one broken
# toolchain does not take the other scenarios with it; the job fails anyway.
if [ "${#failed[@]}" -gt 0 ]; then
  echo "build_peers: failed: ${failed[*]}" >&2
  exit 1
fi
