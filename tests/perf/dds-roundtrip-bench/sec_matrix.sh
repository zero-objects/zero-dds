#!/usr/bin/env bash
# 4x4 DDS-Security 1.2 Cross-Vendor Roundtrip-Matrix.
#
# Vendoren: cyclone, fastdds, opendds, zerodds (RTI ausgelassen —
# Security-Plugin braucht eine Pro/LM-Lizenz, die wir bewusst nicht
# kaufen; dokumentiert als license-blocked).
#
# Jede Zelle: pong_vendor (Echo) + ping_vendor (Messung) auf domain 200
# (= governance.xml domain_rule) mit DDS-Security aktiv. Sequentiell,
# Prozess-Kill + settle zwischen Zellen gegen Discovery-State-Bleed.
#
# Asset-Tree: per Default das in-repo `security/`-Verzeichnis neben diesem
# Skript (CA-self-signed Variant A — von allen vier Vendoren akzeptiert seit
# dem CmsPkcs7Verifier-self-trust-Fix). Die certs + CMS-`.p7s` werden von
# `security/gen.sh` erzeugt (gitignored); einmal `bash security/gen.sh`
# laufen lassen, dann ist die Matrix aus einem sauberen Checkout reproduzierbar.
#
# Aufruf:  bash sec_matrix.sh [BUILD_DIR] [SEC_DIR] [SAMPLES]
#   BUILD_DIR  cmake-Build der Roundtrip-Apps (Default: build-sec)
#   SEC_DIR    Asset-Tree (Default: <script-dir>/security)
#   ZERODDS_LIB_DIR  env: Pfad zur libzerodds_c_api (Default: target/release des Repos)

set -u
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILD="${1:-build-sec}"
SECDIR="${2:-$HERE/security}"
SAMPLES="${3:-2000}"
WARMUP=200
PAYLOAD=64
DOMAIN=200
VENDORS=(cyclone fastdds opendds zerodds)
DYLIB_DIR="$(cd "$(dirname "$BUILD")" && pwd)"

# Startet einen Roundtrip-Prozess in einer Rolle (ping|pong) fuer einen
# Vendor mit Security. Echo der PID. $1=vendor $2=role $3=logfile [$4=ping-args]
start_proc() {
    local vendor="$1" role="$2" log="$3"; shift 3
    local bin="$BUILD/${vendor}-roundtrip"
    local -a env=(ZERODDS_BENCH_DOMAIN=$DOMAIN)
    case "$vendor" in
        cyclone)
            env+=("CYCLONEDDS_URI=file://${SECDIR}/cyclone_security_${role}.xml")
            ;;
        fastdds|zerodds)
            env+=(ZERODDS_BENCH_SECURITY=1 ZERODDS_BENCH_SEC_DIR=$SECDIR ZERODDS_BENCH_SEC_NAME=$role)
            ;;
        opendds)
            env+=(ZERODDS_BENCH_SECURITY=1 ZERODDS_BENCH_SEC_DIR=$SECDIR ZERODDS_BENCH_SEC_NAME=$role)
            ;;
    esac
    # zerodds-roundtrip braucht die secure-dylib im library-path.
    # Default: target/release des Repos (4 Ebenen ueber diesem Skript); per
    # ZERODDS_LIB_DIR ueberschreibbar (z.B. ein dediziertes Build-Verzeichnis).
    local zlib="${ZERODDS_LIB_DIR:-$HERE/../../../target/release}"
    [ "$vendor" = zerodds ] && env+=("LD_LIBRARY_PATH=${zlib}:${LD_LIBRARY_PATH:-}")

    # OpenDDS braucht die security-ini via -DCPSConfigFile.
    local -a appargs=("$@")
    if [ "$vendor" = opendds ]; then
        appargs+=(-DCPSConfigFile "$(dirname "$BUILD")/opendds_rtps_sec.ini")
    fi

    env "${env[@]}" "$bin" "${appargs[@]}" > "$log" 2>&1 &
    echo $!
}

run_cell() {
    local ping_v="$1" pong_v="$2"
    local plog="/tmp/sec_${ping_v}_${pong_v}_pong.log"
    local glog="/tmp/sec_${ping_v}_${pong_v}_ping.log"
    pkill -f -- "-roundtrip" 2>/dev/null; sleep 1

    local pong_pid; pong_pid=$(start_proc "$pong_v" pong "$plog" pong 20)
    sleep 2
    local ping_pid; ping_pid=$(start_proc "$ping_v" ping "$glog" ping --payload $PAYLOAD --warmup $WARMUP --samples $SAMPLES)

    # ping max 60s warten
    for i in $(seq 1 60); do kill -0 "$ping_pid" 2>/dev/null || break; sleep 1; done
    kill "$pong_pid" 2>/dev/null; pkill -f -- "-roundtrip" 2>/dev/null; sleep 1

    # quantile-Zeile extrahieren (alle apps printen "payload=.. p50=..")
    local res; res=$(grep -hoE "p50=[0-9.]+us" "$glog" 2>/dev/null | head -1)
    if [ -z "$res" ]; then
        # Fehlerursache klassifizieren
        if grep -qiE "PKCS7|permission|security|auth" "$glog" "$plog" 2>/dev/null; then
            res="SEC_FAIL"
        elif grep -qiE "timeout|wait_for" "$glog" 2>/dev/null; then
            res="NO_MATCH"
        else
            res="FAIL"
        fi
    fi
    printf "%-9s -> %-9s : %s\n" "$ping_v" "$pong_v" "$res"
}

echo "=== 4x4 DDS-Security Cross-Vendor Matrix (domain $DOMAIN, n=$SAMPLES) ==="
echo "ping \\ pong"
for pv in "${VENDORS[@]}"; do
    for gv in "${VENDORS[@]}"; do
        run_cell "$pv" "$gv"
    done
done
echo "=== done ==="
