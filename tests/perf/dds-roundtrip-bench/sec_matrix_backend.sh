#!/usr/bin/env bash
# Run the 4x4 DDS-Security cross-vendor roundtrip matrix (sec_matrix.sh) with a
# chosen ZeroDDS crypto backend (D1a aws-lc / D1b wolfcrypt).
#
# The vendor apps are backend-agnostic; only the ZeroDDS side changes. This
# wrapper rebuilds libzerodds_c_api with the requested backend feature and points
# sec_matrix.sh at it via ZERODDS_LIB_DIR.
#
# Usage:  bash sec_matrix_backend.sh <ring|fips|wolfcrypt> [SAMPLES]
#
#   ring       default ring backend (baseline)
#   fips       aws-lc-rs / AWS-LC (FIPS-140-3-validatable); links no ring
#   wolfcrypt  wolfSSL/wolfCrypt; requires WOLFSSL_SRC -> a wolfSSL 5.9.x source
#              tree (5.9.0/5.9.1: only releases with both wolfcrypt/src/dilithium.c
#              and src/pk_ec.c that wolfssl-src's file list needs). A system
#              libwolfssl will NOT link (missing PKCS5_PBKDF2_HMAC openssl-compat
#              macro).
#
# Expectation: AES-GCM/HMAC/HKDF wire bytes are backend-independent, so every cell
# stays green across backends; the latency column shows the pure crypto-path cost.
set -eu
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/../../.." && pwd)"
BACKEND="${1:?usage: sec_matrix_backend.sh <ring|fips|wolfcrypt> [SAMPLES]}"
SAMPLES="${2:-2000}"

case "$BACKEND" in
    ring)      FEATURES="security" ;;
    fips)      FEATURES="fips" ;;
    wolfcrypt)
        FEATURES="wolfcrypt"
        : "${WOLFSSL_SRC:?wolfcrypt backend needs WOLFSSL_SRC pointing at a wolfSSL 5.9.x source tree}"
        ;;
    *) echo "unknown backend: $BACKEND (want ring|fips|wolfcrypt)" >&2; exit 2 ;;
esac

echo ">>> building libzerodds_c_api (--release --features $FEATURES) for backend=$BACKEND"
( cd "$REPO" && cargo build -p zerodds-c-api --release --features "$FEATURES" )

# Build the per-vendor roundtrip apps into a backend-specific dir. The ZeroDDS
# app links $REPO/target/release/libzerodds.so (the backend just built); the
# cyclone/fastdds/opendds apps are backend-agnostic.
BUILD="build-sec-$BACKEND"
echo ">>> cmake-building the roundtrip apps into $BUILD"
(
  cd "$HERE"
  export PATH="/opt/cyclone/bin:/opt/fastdds/bin:/opt/fastdds/scripts:$PATH"
  cmake -S . -B "$BUILD" -DOPENDDS_ROOT="${OPENDDS_ROOT:-/opt/opendds-secure}" -DCMAKE_BUILD_TYPE=Release
  cmake --build "$BUILD" -j"$(nproc)"
)

export ZERODDS_LIB_DIR="$REPO/target/release"
export LD_LIBRARY_PATH="$REPO/target/release:/opt/cyclone/lib:/opt/fastdds/lib:/opt/opendds-secure/lib:${LD_LIBRARY_PATH:-}"
echo ">>> running sec_matrix.sh (backend=$BACKEND, samples=$SAMPLES, lib=$ZERODDS_LIB_DIR)"
# IMPORTANT: pass a RELATIVE SECDIR ("security"), NOT an absolute path. sec_matrix.sh
# does `pkill -f -- -roundtrip`, and an absolute SECDIR contains the directory name
# "dds-roundtrip-bench" which matches that pattern -> the matrix would SIGTERM itself.
cd "$HERE"
exec bash sec_matrix.sh "$BUILD" security "$SAMPLES"
