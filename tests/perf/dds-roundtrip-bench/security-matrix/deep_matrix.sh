#!/usr/bin/env bash
# Deep cross-vendor security matrix: profiles (recombination table of the
# governance options) × 4x4 vendor pairs (cyclone/fastdds/opendds/zerodds,
# both roles), per-vendor assets from gen_profile.sh. RTI left out (licence).
#
# REPRODUCIBLE from any checkout — paths derived from the script location,
# overridable via env:
#   PROFILES_DIR  gen_profile.sh output    (default: ./profiles)
#   BUILD         cmake roundtrip binaries (default: ../build-sec)
#   DYLIB         libzerodds.so directory  (default: <repo>/target/release)
#   INI           opendds security INI     (default: ../opendds_rtps_sec.ini)
#   DOMAIN        RTPS domain ID           (default: 200) — must be the same one
#                 with which gen_profile.sh generated the governance.
#   VENDORS       vendor list              (default: cyclone fastdds opendds zerodds)
#   PAIRS         explicit pairs "ping:pong ..." instead of the full cross product
#
# Usage: deep_matrix.sh [SAMPLES] [profile1 profile2 ...]
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
read -r -a VENDORS <<< "${VENDORS:-cyclone fastdds opendds zerodds}"
read -r -a PAIRS <<< "${PAIRS:-}"
DOMAIN="${DOMAIN:-200}"

# --- Domain isolation ------------------------------------------------------
# A measurement on a domain on which someone is already talking also measures
# that participant's discovery. The RTPS port band of the domain (DDSI-RTPS 2.5 §9.6.1.1:
# PB=7400, DG=250, PG=2, d0=0 d1=10 d2=1 d3=11) is therefore checked BEFORE the first
# process. Occupied -> abort, do not measure. The check looks at
# bound UDP ports of the host, not at traffic: a foreign participant
# on this domain necessarily holds at least the SPDP multicast port.
DOMAIN_PB=$(( 7400 + 250 * DOMAIN ))
# Checked are the ports that a participant of this domain necessarily holds:
# the two multicast ports d0/d2 and the unicast slots d1/d3 of the first 20
# participant_ids. Checking the whole band of 250 produces false alarms as soon as
# it reaches into the ephemeral range of the host (>= 32768) — any
# foreign socket there would then have aborted the run.
domain_band_busy() {
  local ports="$DOMAIN_PB $(( DOMAIN_PB + 1 ))" pid
  for pid in $(seq 0 19); do
    ports="$ports $(( DOMAIN_PB + 10 + 2 * pid )) $(( DOMAIN_PB + 11 + 2 * pid ))"
  done
  local bound; bound=$(ss -ulan 2>/dev/null | awk '{print $5}' | grep -oE '[0-9]+$' | sort -nu)
  comm -12 <(printf '%s\n' $ports | sort -u) <(printf '%s\n' $bound) | tr '\n' ' '
}
busy="$(domain_band_busy)"
if [ -n "$busy" ]; then
  echo "ABORT: domain $DOMAIN (RTPS ports from $DOMAIN_PB) is occupied: $busy" >&2
  echo "       Choose another domain (DOMAIN=<id>) and regenerate the profiles with it." >&2
  exit 3
fi
echo "== Domain $DOMAIN, RTPS ports from $DOMAIN_PB free =="

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
    # ZeroDDS runs its reliable secure SPDP channel (0xff0101), which Fast DDS
    # waits for, by default on every secured participant.
    # Pairs with Fast DDS: Fast DDS writes the algorithm names c.dsign_algo and
    # c.kagree_algo of its handshake request without the terminating NUL
    # (DDS-Security 1.2 section 7.3.3.1 asks for it) and compares the names it
    # gets back. ZeroDDS answers in the form of the request only under this
    # named setting; its default is the form of the spec. The Fast DDS column
    # runs with the setting, the other columns with the defaults.
    [ "$peer" = fastdds ] && e+=(ZERODDS_SEC_ALGO_NAME_NUL=mirror)
    # Pairs with Cyclone DDS, Fast DDS and OpenDDS: DDS-Security 1.2 Table 25
    # and Table 26 name the algorithms ECDSA+P256+SHA256, ECDHE-CEUM+P256 and
    # DHE+MODP-2048-256, and mark ECDSA-SHA256, ECDH+prime256v1-CEUM and
    # DH+MODP-2048-256 "deprecated in v 1.2". ZeroDDS sends the names of the
    # tables by default. The three vendors announce DDS:Auth:PKI-DH:1.0 and
    # their handshake code compares c.dsign_algo and c.kagree_algo with the
    # names of 1.1 (sources read, not measured here), so their columns run
    # with the named setting that sends the names of 1.1.
    case "$peer" in cyclone|fastdds|opendds) e+=(ZERODDS_SEC_ALGORITHM_NAMES=1.1) ;; esac
    # Pairs with Fast DDS and OpenDDS: dds.cert.sn, dds.ca.sn and
    # dds.perm_ca.sn carry the subject name, which DDS-Security 1.2 section
    # 9.4.1.2.3 writes as RFC 4514 (the default of ZeroDDS). Fast DDS 3.6
    # compares a received dds.perm_ca.sn with X509_NAME_oneline of its own
    # Permissions CA (Permissions.cpp:1007), OpenDDS a received dds.ca.sn
    # with the XN_FLAG_ONELINE string of its own CA
    # (AuthenticationBuiltInImpl.cpp:259), so these columns run with the named
    # setting that writes the form of the peer. Cyclone DDS writes
    # X509_NAME_oneline and is not known to compare a foreign value: its
    # column runs with the default.
    case "$peer" in
      fastdds) e+=(ZERODDS_SEC_SUBJECT_NAME_FORM=x509_name_oneline) ;;
      opendds) e+=(ZERODDS_SEC_SUBJECT_NAME_FORM=xn_flag_oneline) ;;
    esac
    # Pairs with OpenDDS: under a governance with origin authentication
    # (oa-sign, oa-enc) OpenDDS 3.34 makes no receiver-specific keys
    # (CryptoBuiltInImpl.cpp: "origin auth is not implemented";
    # receiver_specific_key_id 0). ZeroDDS refuses such a key by default, as
    # DDS-Security 1.2 section 10.5.3.3 Table 80 decode_rtps_message asks
    # (measured: every message of OpenDDS refused, "the transformation_kind is
    # not the one expected (origin authentication)"). The OpenDDS column runs
    # with the named setting that verifies such a key with the common MAC
    # alone (security.origin_authentication_fallback=accept).
    [ "$peer" = opendds ] && e+=(ZERODDS_SEC_ORIGIN_AUTHENTICATION_FALLBACK=accept)

    # Fast DDS (and OpenDDS, RTI Connext) send the ACKNACK of a reader that
    # holds nothing with bitmapBase 0 and numBits 0, which DDSI-RTPS 2.5
    # section 8.3.5.5 makes invalid. ZeroDDS by default drops the rest of such
    # a datagram (section 8.3.4.1 rule 6), and with it the SEDP data that
    # follows (measured: sec-diag16/17; every red zerodds->fastdds cell had
    # rejections with left_out > 0, every green cell none). The ZeroDDS process
    # of these pairs accepts it (wire.accept_empty_acknack_base_zero).
    case "$peer" in fastdds|opendds|rti) e+=(ZERODDS_WIRE_ACCEPT_EMPTY_ACKNACK_BASE_ZERO=1) ;; esac
    # Cyclone DDS and RTI Connext write PL_CDR1 parameters with the length
    # rounded up to four (DDSI-RTPS 2.5 section 9.4.2.11; recorded rows,
    # ast-layout Dialect::Cyclone and Dialect::Rti); the default reader of
    # ZeroDDS refuses that form (XTypes 1.3 section 7.4.1.2.2). The ZeroDDS
    # process of these pairs reads it (wire.pl_cdr1_reading=tolerant).
    case "$peer" in cyclone|rti) e+=(ZERODDS_WIRE_PL_CDR1_READING=tolerant) ;; esac
  fi
  env "${e[@]}" "$bin" "${a[@]}" > "$log" 2>&1 & echo $!
}

# The pkill pattern is anchored on $BUILD and not the bare string
# "-roundtrip": the bench directory is named dds-roundtrip-bench, so a
# bare -f match hits every command line carrying an absolute path into
# this tree -- including deep_matrix.sh itself when run_deep_matrix.sh
# invokes it as "$HERE/deep_matrix.sh". That is why the documented
# one-command entry point died at the first cell while calling
# deep_matrix.sh by hand from this directory worked.
run_cell() { # $1=ping_v $2=pong_v $3=profile
  local pv="$1" gv="$2" prof="$3"
  local pl="/tmp/dm_${prof}_${pv}_${gv}_pong.log" gl="/tmp/dm_${prof}_${pv}_${gv}_ping.log"
  pkill -9 -f -- "$BUILD/[a-z]*-roundtrip" 2>/dev/null; sleep 1
  local pp; pp=$(start_proc "$gv" pong "$prof" "$pl" "$pv" pong 20); sleep 2
  local gp; gp=$(start_proc "$pv" ping "$prof" "$gl" "$gv" ping --payload 64 --warmup 50 --samples $SAMPLES)
  for i in $(seq 1 45); do kill -0 "$gp" 2>/dev/null || break; sleep 1; done
  kill "$pp" 2>/dev/null; pkill -9 -f -- "$BUILD/[a-z]*-roundtrip" 2>/dev/null; sleep 1
  # A green cell is read from the number of delivered samples, not from the
  # exit code: `n=` is the number of echoes that ping got back within its
  # 50 ms window (`--samples` is the upper bound). A
  # run with p50 but n far below SAMPLES partially failed to deliver.
  local p50 n r
  p50=$(grep -hoE "p50=[0-9.]+us" "$gl" 2>/dev/null | head -1)
  n=$(grep -hoE "\<n=[0-9]+" "$gl" 2>/dev/null | head -1)
  r=""
  [ -n "$p50" ] && r="$p50 ${n:-n=?}"
  if [ -z "$r" ]; then
    if grep -qiE "PKCS7|permission|governance|auth|security|No governance" "$gl" "$pl" 2>/dev/null; then r=SEC_FAIL
    elif grep -qiE "timeout|wait_for|no samples|pub=0|sub=0" "$gl" 2>/dev/null; then r=NO_MATCH
    else r=FAIL; fi
  fi
  printf "%-9s -> %-9s : %s\n" "$pv" "$gv" "$r"
}

for prof in "${PROFILES[@]}"; do
  echo "######## PROFILE: $prof ########"
  if [ "${#PAIRS[@]}" -gt 0 ]; then
    for p in "${PAIRS[@]}"; do run_cell "${p%%:*}" "${p##*:}" "$prof"; done
  else
    for pv in "${VENDORS[@]}"; do for gv in "${VENDORS[@]}"; do run_cell "$pv" "$gv" "$prof"; done; done
  fi
done
echo "=== deep_matrix done ==="
