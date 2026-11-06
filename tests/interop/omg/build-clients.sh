#!/usr/bin/env bash
# Build ZeroDDS' OMG suite clients and copy them under the names the
# harnesses read a product from (`<product>_shape_main_linux`,
# `<product>_test_main_linux`).
#
#   build-clients.sh <out-dir> <package>...
#
# One cargo invocation per package: `cargo build -p a -p b` unifies the
# features of both and builds binaries neither ships. Release: the suites
# time the samples (a 33 ms write period, lifespans of 250 ms). Measured: 90 s
# for shape_main in an empty target directory of the GitLab runner, 12 s with
# nothing changed (pipelines 8487 and 8489).
set -euo pipefail
out=$1; shift
mkdir -p "$out"
target=${CARGO_TARGET_DIR:-target}
for package in "$@"; do
  case "$package" in
    zerodds-omg-shape-main)       bin=shape_main;        name=zerodds_shape_main_linux ;;
    zerodds-omg-xtypes-test-main) bin=zerodds_test_main; name=zerodds_test_main_linux ;;
    zerodds-omg-security-main)    bin=shapetype_main;    name=zerodds_security_main_linux ;;
    *) echo "build-clients.sh: no suite client named $package" >&2; exit 2 ;;
  esac
  start=$(date +%s)
  cargo build --locked --release -p "$package" --bin "$bin"
  cp "$target/release/$bin" "$out/$name"
  echo "== $package: $out/$name, $(( $(date +%s) - start )) s"
done
# The suite jobs check out nothing (GIT_STRATEGY none: a clone took up to
# 32 s of a loaded runner's 5 minutes, pipeline 8510); what they run from
# the tree comes with the clients, at the same relative paths.
mkdir -p "$out/repo/tests/interop" "$out/repo/tools/omg-security-main" "$out/repo/ci"
cp -r tests/interop/omg "$out/repo/tests/interop/"
cp ci/job_io.sh "$out/repo/ci/"
cp tools/omg-security-main/run_matrix.py tools/omg-security-main/compatibility_matrix.tsv \
   "$out/repo/tools/omg-security-main/"
