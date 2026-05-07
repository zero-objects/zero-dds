#!/usr/bin/env bash
# Seed die cargo-fuzz-Corpora aus den Cyclone-Fixtures.
#
# Hex-Fixtures (mit `#`-Kommentarzeilen, Whitespace) werden zu Binary-
# Bytes dekodiert und in fuzz/corpus/<target>/ abgelegt. Coverage-
# guided Fuzzer nutzen diese als Startpunkt — findet Panic-Pfade, die
# nahe an realem RTPS-Traffic liegen.
#
# Aufruf: bash crates/rtps/fuzz/scripts/seed-corpus.sh
set -euo pipefail

cd "$(dirname "$0")/.."
FUZZ_ROOT=$(pwd)
FIXTURES="${FUZZ_ROOT}/../tests/fixtures/cyclone"

hex_to_bin() {
    local src=$1 dst=$2
    mkdir -p "$(dirname "$dst")"
    # Zeilen-Kommentare (`#`) und Whitespace entfernen, dann xxd -r -p
    sed -e 's/#.*//' -e 's/[[:space:]]//g' "$src" | xxd -r -p > "$dst"
}

# decode_datagram bekommt ganze Datagrams — alle drei Fixtures
for f in "${FIXTURES}"/*.hex; do
    name=$(basename "$f" .hex)
    hex_to_bin "$f" "${FUZZ_ROOT}/corpus/decode_datagram/cyclone_${name}"
done

# submessage_decoders bekommt je 1 Byte ID-Selector + body. Wir hashen
# die Submessage-Body-Bytes direkt rein (Selector=0 → DataSubmessage).
# Realistisch wuerden wir die Submessages erst aus den Datagrams
# extrahieren — fuer ein Bootstrap-Corpus reicht das Ganze-Datagram
# als "zufaellige Bytes mit RTPS-Header" schon.
for f in "${FIXTURES}"/*.hex; do
    name=$(basename "$f" .hex)
    hex_to_bin "$f" "${FUZZ_ROOT}/corpus/submessage_decoders/cyclone_${name}"
done

# fragment_assembler: wir haben keine DATA_FRAG-Fixture (offener Punkt
# fuer WP 1.4). Stattdessen konstruieren wir ein minimales synthetisches
# Seed: 20 Null-Bytes + "hello" — zwingt den Fuzzer nicht in eine
# Ecke, gibt ihm aber valide Byte-Laengen als Startpunkt.
mkdir -p "${FUZZ_ROOT}/corpus/fragment_assembler"
printf '\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0hello' \
    > "${FUZZ_ROOT}/corpus/fragment_assembler/seed_minimal"

echo "Seeded corpora under ${FUZZ_ROOT}/corpus/:"
find "${FUZZ_ROOT}/corpus" -type f | sed "s|${FUZZ_ROOT}/||"
