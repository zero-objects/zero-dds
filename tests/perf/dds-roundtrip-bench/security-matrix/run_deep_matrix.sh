#!/usr/bin/env bash
# Ein-Befehl-Orchestrator fuer die volle Cross-Vendor-DDS-Security-Matrix.
# REPRODUZIERBAR aus jedem Checkout (Linux, Vendor-Stack + build-sec/ vorhanden):
#
#   bash run_deep_matrix.sh [SAMPLES]
#
# Schritte:
#   1. cert-Tree sicherstellen  (../security/gen.sh, wenn certs/ fehlt)
#   2. die 13 Profile generieren (gen_profile.sh, Tabelle unten)
#   3. deep_matrix.sh ueber alle 13 Profile × 4×4 Vendor-Paare
#
# Voraussetzung: build-sec/{cyclone,fastdds,opendds,zerodds}-roundtrip +
# <repo>/target/release/libzerodds.so (cargo build --release -p zerodds-c-api
# --features security). Siehe README.md.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SECDIR="$(cd "$HERE/../security" && pwd)"
SAMPLES="${1:-500}"

# 1) cert-Tree (einmalig) — gen.sh erzeugt certs/ + permissions-Signaturen.
if [ ! -f "$SECDIR/certs/permissions_ca.pem" ]; then
  echo "== cert-Tree fehlt -> ../security/gen.sh =="
  ( cd "$SECDIR" && bash gen.sh >/dev/null )
fi

# 2) Die 13 Profile (Tabelle = internal/security/cross-vendor-secure-interop-matrix.md
#    §1). Spalten = gen_profile.sh-Args:
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
"

echo "== generiere 13 Profile =="
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
