#!/usr/bin/env bash
# Seed the cargo-fuzz corpora from the Cyclone fixtures.
#
# Hex fixtures (with `#` comment lines, whitespace) are decoded to binary
# bytes and placed in fuzz/corpus/<target>/. Coverage-
# guided fuzzers use these as a starting point — finding panic paths that
# are close to real RTPS traffic.
#
# Invocation: bash crates/rtps/fuzz/scripts/seed-corpus.sh
set -euo pipefail

cd "$(dirname "$0")/.."
FUZZ_ROOT=$(pwd)
FIXTURES="${FUZZ_ROOT}/../tests/fixtures/cyclone"

hex_to_bin() {
    local src=$1 dst=$2
    mkdir -p "$(dirname "$dst")"
    # Strip line comments (`#`) and whitespace, then xxd -r -p
    sed -e 's/#.*//' -e 's/[[:space:]]//g' "$src" | xxd -r -p > "$dst"
}

# decode_datagram gets whole datagrams — all three fixtures
for f in "${FIXTURES}"/*.hex; do
    name=$(basename "$f" .hex)
    hex_to_bin "$f" "${FUZZ_ROOT}/corpus/decode_datagram/cyclone_${name}"
done

# submessage_decoders gets a 1-byte ID selector + body each. We hash
# the submessage body bytes directly in (selector=0 → DataSubmessage).
# Realistically we would first extract the submessages from the datagrams
# — for a bootstrap corpus the whole datagram
# as "random bytes with an RTPS header" is already enough.
for f in "${FIXTURES}"/*.hex; do
    name=$(basename "$f" .hex)
    hex_to_bin "$f" "${FUZZ_ROOT}/corpus/submessage_decoders/cyclone_${name}"
done

# fragment_assembler: we have no DATA_FRAG fixture (open item
# for WP 1.4). Instead we construct a minimal synthetic
# seed: 20 null bytes + "hello" — does not force the fuzzer into a
# corner, but gives it valid byte lengths as a starting point.
mkdir -p "${FUZZ_ROOT}/corpus/fragment_assembler"
printf '\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0hello' \
    > "${FUZZ_ROOT}/corpus/fragment_assembler/seed_minimal"

echo "Seeded corpora under ${FUZZ_ROOT}/corpus/:"
find "${FUZZ_ROOT}/corpus" -type f | sed "s|${FUZZ_ROOT}/||"
