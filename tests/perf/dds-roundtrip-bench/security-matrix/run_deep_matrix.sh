#!/usr/bin/env bash
# One-command orchestrator for the full cross-vendor DDS-Security matrix.
# REPRODUCIBLE from any checkout (Linux, vendor stack + build-sec/ present):
#
#   bash run_deep_matrix.sh [SAMPLES]
#
# Steps:
#   1. ensure the cert tree     (../security/gen.sh, if certs/ is missing)
#   2. generate the 15 profiles (gen_profile.sh, table below)
#   3. deep_matrix.sh over all 15 profiles × 4×4 vendor pairs
#
# Prerequisite: build-sec/{cyclone,fastdds,opendds,zerodds}-roundtrip +
# <repo>/target/release/libzerodds.so (cargo build --release -p zerodds-c-api
# --features security). See README.md.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SECDIR="$(cd "$HERE/../security" && pwd)"
SAMPLES="${1:-500}"

# 1) cert tree (once) — gen.sh generates certs/ + permissions signatures.
if [ ! -f "$SECDIR/certs/permissions_ca.pem" ]; then
  echo "== cert tree missing -> ../security/gen.sh =="
  ( cd "$SECDIR" && bash gen.sh >/dev/null )
fi

# 2) The 15 profiles (table = internal/security/cross-vendor-secure-interop-matrix.md
#    §1). Columns = gen_profile.sh args:
#      name   disc     liv      rtps     meta     data     join rw  endisc enliv
PROFILES_TABLE="
common-subset  NONE     NONE     NONE     NONE     ENCRYPT  false false false false
data-enc       NONE     NONE     NONE     NONE     ENCRYPT  false false false false
data-sign      NONE     NONE     NONE     NONE     SIGN     false false false false
meta-data-enc  NONE     NONE     NONE     ENCRYPT  ENCRYPT  false false false false
meta-sign-data NONE     NONE     NONE     SIGN     ENCRYPT  false false false false
liv-data-enc   NONE     ENCRYPT  NONE     NONE     ENCRYPT  false false false true
disc-meta-data ENCRYPT  NONE     NONE     ENCRYPT  ENCRYPT  false false true  false
disc-data-enc  ENCRYPT  NONE     NONE     NONE     ENCRYPT  false false true  false
rtps-enc       NONE     NONE     ENCRYPT  NONE     NONE     false false false false
rtps-sign-data NONE     NONE     SIGN     NONE     ENCRYPT  false false false false
all-enc        ENCRYPT  ENCRYPT  ENCRYPT  ENCRYPT  ENCRYPT  true  true  true  true
all-sign       SIGN     SIGN     SIGN     SIGN     SIGN     true  true  true  true
sros2-full     ENCRYPT  ENCRYPT  ENCRYPT  ENCRYPT  ENCRYPT  true  true  true  true
oa-sign        SIGN_WITH_ORIGIN_AUTHENTICATION    SIGN_WITH_ORIGIN_AUTHENTICATION    SIGN_WITH_ORIGIN_AUTHENTICATION    SIGN_WITH_ORIGIN_AUTHENTICATION    ENCRYPT  false false true  true
oa-enc         ENCRYPT_WITH_ORIGIN_AUTHENTICATION ENCRYPT_WITH_ORIGIN_AUTHENTICATION ENCRYPT_WITH_ORIGIN_AUTHENTICATION ENCRYPT_WITH_ORIGIN_AUTHENTICATION ENCRYPT  false false true  true
"
# oa-sign / oa-enc: the origin-authentication kinds (DDS-Security 1.2
# 10.4.1.2.5, ProtectionKind) for discovery, liveliness, RTPS and metadata.
# data_protection_kind is a BasicProtectionKind and takes no such kind.

echo "== generating 15 profiles =="
NAMES=()
while read -r name disc liv rtps meta data join rw endisc enliv; do
  [ -z "${name:-}" ] && continue
  bash "$HERE/gen_profile.sh" "$name" "$disc" "$liv" "$rtps" "$meta" "$data" "$join" "$rw" "$endisc" "$enliv" >/dev/null
  NAMES+=("$name")
done <<< "$PROFILES_TABLE"
echo "   -> ${NAMES[*]}"

# 3) Matrix
echo "== deep_matrix (SAMPLES=$SAMPLES) =="
bash "$HERE/deep_matrix.sh" "$SAMPLES" "${NAMES[@]}"
