#!/usr/bin/env bash
# Build `hdds_robot`, the HDDS client of the #28 interop gate.
#
#   build.sh release   HDDS at the tag of Cargo.toml, versions from Cargo.lock
#   build.sh master    HDDS at HDDS_MASTER_REV below
#
# Like the Fast DDS and RTI cells, this compiles OUR client; HDDS comes in
# as a Cargo git dependency. The binary lands at build/<variant>/hdds_robot.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# `master` of git.hdds.io/hdds/hdds, committed 2026-05-16. It is an ancestor
# of the tag v1.1.5 (2026-05-24).
HDDS_MASTER_REV=8701c0b6ad8c44b6b9328dee6ff7b8ac0d4c20d1

VARIANT="${1:-release}"
OUT="$HERE/build/$VARIANT"
mkdir -p "$OUT"
export HDDS_ROBOT_IDL="$HERE/../fastdds/robot.idl"

case "$VARIANT" in
  release)
    cargo build --release --locked --manifest-path "$HERE/Cargo.toml" --target-dir "$HERE/target"
    cp "$HERE/target/release/hdds_robot" "$OUT/"
    ;;
  master)
    # A copy of the crate whose HDDS dependency names the master commit.
    # Cargo.lock goes along, so only the HDDS packages resolve anew.
    src="$OUT/crate"
    rm -rf "$src"
    mkdir -p "$src"
    cp -R "$HERE/src" "$HERE/build.rs" "$HERE/Cargo.lock" "$src/"
    sed "s|tag = \"v1.1.5\"|rev = \"$HDDS_MASTER_REV\"|" "$HERE/Cargo.toml" >"$src/Cargo.toml"
    grep -q "rev = \"$HDDS_MASTER_REV\"" "$src/Cargo.toml" \
      || { echo "build.sh: the HDDS dependency of Cargo.toml was not rewritten" >&2; exit 1; }
    cargo build --release --manifest-path "$src/Cargo.toml" --target-dir "$OUT/target"
    cp "$OUT/target/release/hdds_robot" "$OUT/"
    ;;
  *)
    echo "usage: build.sh <release|master>" >&2
    exit 2
    ;;
esac
"$OUT/hdds_robot" version
