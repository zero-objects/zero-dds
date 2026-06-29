#!/usr/bin/env bash
# Tiefe Cross-Vendor-Security-Matrix: Profile (Rekombinationstabelle der
# governance-Optionen) × 4x4 Vendor-Paare (cyclone/fastdds/opendds/zerodds,
# beide Rollen), per-Vendor-Assets aus gen_profile.sh. RTI ausgelassen (Lizenz).
#
# REPRODUZIERBAR aus jedem Checkout — Pfade aus der Script-Location abgeleitet,
# per env ueberschreibbar:
#   PROFILES_DIR  gen_profile.sh-Ausgabe   (Default: ./profiles)
#   BUILD         cmake-Roundtrip-Binaries (Default: ../build-sec)
#   DYLIB         libzerodds.so-Verzeichnis(Default: <repo>/target/release)
#   INI           opendds-Security-INI     (Default: ../opendds_rtps_sec.ini)
#
# Aufruf: deep_matrix.sh [SAMPLES] [profil1 profil2 ...]
set -u
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BENCH="$(cd "$HERE/.." && pwd)"
REPO="$(cd "$BENCH/../../.." && pwd)"
PROFILES_DIR="${PROFILES_DIR:-$HERE/profiles}"
BUILD="${BUILD:-$BENCH/build-sec}"
DYLIB="${DYLIB:-$REPO/target/release}"
INI="${INI:-$BENCH/opendds_rtps_sec.ini}"
SAMPLES="${1:-200}"; shift || true
PROFILES=("$@")
VENDORS=(cyclone fastdds opendds zerodds)
DOMAIN=200

start_proc() { # $1=vendor $2=role $3=profile $4=log $5=peer ; rest=appargs
  local v="$1" role="$2" prof="$3" log="$4" peer="$5"; shift 5
  local bin="$BUILD/${v}-roundtrip"; local -a e=(ZERODDS_BENCH_DOMAIN=$DOMAIN); local -a a=("$@")
  case "$v" in
    cyclone) e+=("CYCLONEDDS_URI=file://$PROFILES_DIR/$prof/cyclone_${role}.xml") ;;
    fastdds|zerodds) e+=(ZERODDS_BENCH_SECURITY=1 ZERODDS_BENCH_SEC_DIR=$PROFILES_DIR/$prof/text ZERODDS_BENCH_SEC_NAME=$role) ;;
    opendds) e+=(ZERODDS_BENCH_SECURITY=1 ZERODDS_BENCH_SEC_DIR=$PROFILES_DIR/$prof/text ZERODDS_BENCH_SEC_NAME=$role); a+=(-DCPSConfigFile "$INI") ;;
  esac
  if [ "$v" = zerodds ]; then
    e+=("LD_LIBRARY_PATH=$DYLIB:${LD_LIBRARY_PATH:-}")
    # FastDDS gated die Crypto-Token-Reziprokation auf ZeroDDS' reliablem
    # secure-SPDP-Kanal (0xff0101); ohne diesen sieht FastDDS ZeroDDS dort nie
    # und reziprokiert keine datawriter/datareader-Tokens -> NO_MATCH. Nur fuer
    # den fastdds-Peer einschalten — fuer rtps-enc/discovery=NONE waere
    # secure-SPDP spec-falsch (erzwingt ff0003/ff0004-Entities).
    [ "$peer" = fastdds ] && e+=(ZERODDS_SECURE_SPDP=1)
  fi
  env "${e[@]}" "$bin" "${a[@]}" > "$log" 2>&1 & echo $!
}

run_cell() { # $1=ping_v $2=pong_v $3=profile
  local pv="$1" gv="$2" prof="$3"
  local pl="/tmp/dm_${prof}_${pv}_${gv}_pong.log" gl="/tmp/dm_${prof}_${pv}_${gv}_ping.log"
  pkill -9 -f -- "-roundtrip" 2>/dev/null; sleep 1
  local pp; pp=$(start_proc "$gv" pong "$prof" "$pl" "$pv" pong 20); sleep 2
  local gp; gp=$(start_proc "$pv" ping "$prof" "$gl" "$gv" ping --payload 64 --warmup 50 --samples $SAMPLES)
  for i in $(seq 1 45); do kill -0 "$gp" 2>/dev/null || break; sleep 1; done
  kill "$pp" 2>/dev/null; pkill -9 -f -- "-roundtrip" 2>/dev/null; sleep 1
  local r; r=$(grep -hoE "p50=[0-9.]+us" "$gl" 2>/dev/null | head -1)
  if [ -z "$r" ]; then
    if grep -qiE "PKCS7|permission|governance|auth|security|No governance" "$gl" "$pl" 2>/dev/null; then r=SEC_FAIL
    elif grep -qiE "timeout|wait_for|no samples|pub=0|sub=0" "$gl" 2>/dev/null; then r=NO_MATCH
    else r=FAIL; fi
  fi
  printf "%-9s -> %-9s : %s\n" "$pv" "$gv" "$r"
}

for prof in "${PROFILES[@]}"; do
  echo "######## PROFILE: $prof ########"
  for pv in "${VENDORS[@]}"; do for gv in "${VENDORS[@]}"; do run_cell "$pv" "$gv" "$prof"; done; done
done
echo "=== deep_matrix done ==="
