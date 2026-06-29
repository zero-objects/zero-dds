#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# matrix_sweep.sh — Cross-Vendor Roundtrip-Matrix mit Payload-Sweep.
#
# Faehrt jeden Vendor als ping gegen jeden Vendor als pong (inkl. self,
# 5x5 = 25 Zellen) fuer jede Payload-Groesse 0..8192 Byte in 10%-
# Schritten (11 Punkte) — 275 Zellen. Pro Zelle werden `REPEAT_RUNS`
# unabhaengige Runs gefahren (Default 5), die Zeile mit dem niedrigsten
# p50 wird in die Haupt-CSV uebernommen — "min-of-medians" senkt
# Co-Tenant-Jitter aus dem the virtualisation host (Load typ. 12+ auf 16 Cores) auf
# unter 5%. Alle Runs landen zusaetzlich in `matrix_runs_raw.csv`
# fuer Streuungs-Inspektion.
#
# Aufruf (auf x86-host):
#   tests/perf/dds-roundtrip-bench/matrix_sweep.sh [out_dir]
#   REPEAT_RUNS=N tests/perf/dds-roundtrip-bench/matrix_sweep.sh [out_dir]
#
# Voraussetzung: die fuenf <vendor>-roundtrip-Binaries sind in
# build/ gebaut; Cyclone/Fast-DDS/RTI/OpenDDS installiert (Pfade unten).

set -u

BENCH_DIR="$(cd "$(dirname "$0")" && pwd)"
BUILD_DIR="$BENCH_DIR/build"
ZD_REPO="$(cd "$BENCH_DIR/../../.." && pwd)"
INI="$BENCH_DIR/opendds_rtps.ini"
OUT_DIR="${1:-$BENCH_DIR/matrix-out}"
mkdir -p "$OUT_DIR"
CSV="$OUT_DIR/matrix_results.csv"
RAW_CSV="$OUT_DIR/matrix_runs_raw.csv"
REPORT="$OUT_DIR/matrix_report.md"

SAMPLES=2000
WARMUP=200
REPEAT_RUNS="${REPEAT_RUNS:-5}"
VENDORS=(zerodds cyclone fastdds rti opendds)
# 0..8192 Byte in 10%-Schritten (11 Punkte), gerundet.
PAYLOADS=(0 819 1638 2458 3277 4096 4915 5734 6554 7373 8192)

# --- Vendor-Runtime-Umgebung ---
# Caller-overrideable defaults — wenn schon gesetzt (z.B. macOS-Host
# mit Vendoren in $HOME/dds-prefix), lassen wir's stehen. Nur Linux-
# x86-host fuellt die Default-Pfade aus.
: "${NDDSHOME:=/opt/rti.com/rti_connext_dds-7.7.0}"
: "${RTI_LICENSE_FILE:=$NDDSHOME/rti_license.dat}"
export NDDSHOME RTI_LICENSE_FILE
if [ "$(uname)" = Linux ]; then
    : "${LD_LIBRARY_PATH:=/opt/cyclone/lib:/opt/fastdds/lib:$NDDSHOME/lib/x64Linux4gcc8.5.0:$ZD_REPO/target/release}"
    export LD_LIBRARY_PATH
    # OpenDDS-Service-Participant-Env (DCPSInfoRepo etc.) — best effort.
    # shellcheck disable=SC1091
    . $OPENDDS_SRC/setenv.sh 2>/dev/null || true
elif [ "$(uname)" = Darwin ]; then
    # macOS SIP strippt DYLD_LIBRARY_PATH beim Exec von Apple-signierten
    # Binaries (z.B. /bin/bash). Re-export hier im Script-Body, weil
    # der Bash-Process selbst SIP-protected ist und beim Spawn die env
    # verliert. `DYLD_LIBRARY_PATH_OVERRIDE` ist Caller-Convention, der
    # Wert wird hier auf das echte DYLD_LIBRARY_PATH gespiegelt.
    if [ -n "${DYLD_LIBRARY_PATH_OVERRIDE:-}" ]; then
        export DYLD_LIBRARY_PATH="$DYLD_LIBRARY_PATH_OVERRIDE"
    else
        # Default macOS-Pfade fuer Cyclone/Fast-DDS/RTI im $HOME/dds-prefix
        # bzw. RTI-Install in $HOME/y.
        : "${DYLD_LIBRARY_PATH:=$ZD_REPO/target/release:$HOME/dds-prefix/lib:$HOME/y/rti_connext_dds-7.7.0/lib/arm64Darwin23clang16.0}"
        export DYLD_LIBRARY_PATH
    fi
fi

cd "$BUILD_DIR" || { echo "build dir fehlt: $BUILD_DIR" >&2; exit 1; }

for v in "${VENDORS[@]}"; do
    [ -x "./${v}-roundtrip" ] || { echo "Binary fehlt: ${v}-roundtrip" >&2; exit 1; }
done

# OpenDDS-Binaries brauchen -DCPSConfigFile; die anderen nicht.
extra_args() { [ "$1" = opendds ] && printf '%s' "-DCPSConfigFile $INI"; }

# Fuehrt eine Zelle aus; gibt die ping-Ergebniszeile auf stdout aus
# ("payload=.. n=.. min=.. p50=.. .." oder "no samples" oder leer).
run_cell() {
    local ping=$1 pong=$2 payload=$3
    local pong_log ping_log
    pong_log="$(mktemp)"; ping_log="$(mktemp)"

    # pong im Hintergrund — grosszuegige Runtime, wird nach ping gekillt.
    # shellcheck disable=SC2086
    ./"${pong}-roundtrip" pong 60 $(extra_args "$pong") >"$pong_log" 2>&1 &
    local pong_pid=$!

    # OpenDDS-pong startet langsam — laengeres Discovery-Fenster.
    [ "$pong" = opendds ] && sleep 5 || sleep 3

    # shellcheck disable=SC2086
    timeout 90 ./"${ping}-roundtrip" ping --payload "$payload" \
        --samples "$SAMPLES" --warmup "$WARMUP" $(extra_args "$ping") \
        >"$ping_log" 2>&1

    kill "$pong_pid" 2>/dev/null
    sleep 0.5
    kill -9 "$pong_pid" 2>/dev/null
    wait "$pong_pid" 2>/dev/null

    # macOS multicast/SHM state braucht echten Settle-Gap zwischen Cells
    # — sonst bleiben Sockets in TIME_WAIT haengen und Folge-Pongs scheitern
    # an `Port in use`. 2s reicht fuer Loopback-Cleanup.
    if [ "$(uname)" = Darwin ]; then sleep 2; fi

    grep -E "payload=|no samples" "$ping_log" | head -1
    rm -f "$pong_log" "$ping_log"
}

# Extrahiert ein "key=ZAHL"-Feld aus der Ergebniszeile.
field() { printf '%s' "$1" | grep -oE "$2=[0-9.]+" | head -1 | cut -d= -f2; }

echo "ping_vendor,pong_vendor,payload_bytes,n,min_us,p50_us,p90_us,p99_us,p999_us,max_us,status" >"$CSV"
echo "ping_vendor,pong_vendor,payload_bytes,run_idx,n,min_us,p50_us,p90_us,p99_us,p999_us,max_us,status" >"$RAW_CSV"

total=$(( ${#VENDORS[@]} * ${#VENDORS[@]} * ${#PAYLOADS[@]} ))
idx=0
started="$(date +%s)"

# Faehrt eine Zelle `REPEAT_RUNS`-mal, sammelt alle Run-Zeilen ins
# Raw-CSV und gibt die Zeile mit dem kleinsten p50 aus stdout zurueck
# (oder eine Timeout-Zeile wenn jeder Run scheiterte). Damit ist die
# Haupt-CSV "min-of-medians" — invariant gegen co-tenant-Jitter.
run_cell_repeated() {
    local ping=$1 pong=$2 payload=$3
    local best_line="" best_p50="" line p50 r
    for r in $(seq 1 "$REPEAT_RUNS"); do
        line="$(run_cell "$ping" "$pong" "$payload")"
        if [ -z "$line" ] || ! printf '%s' "$line" | grep -q "payload="; then
            # Retry-Versuch sofort, sonst als timeout/no-samples markieren.
            sleep 1
            line="$(run_cell "$ping" "$pong" "$payload")"
        fi
        if printf '%s' "$line" | grep -q "payload="; then
            p50="$(field "$line" p50)"
            local rn rm rp50 rp90 rp99 rp999 rmx
            rn="$(field "$line" n)"
            rm="$(field "$line" min)"
            rp50="$p50"
            rp90="$(field "$line" p90)"
            rp99="$(field "$line" p99)"
            rp999="$(field "$line" p999)"
            rmx="$(field "$line" max)"
            echo "$ping,$pong,$payload,$r,$rn,$rm,$rp50,$rp90,$rp99,$rp999,$rmx,ok" >>"$RAW_CSV"
            if [ -z "$best_p50" ] || awk "BEGIN{exit !($p50 < $best_p50)}"; then
                best_p50="$p50"
                best_line="$line"
            fi
        else
            local st
            if [ -z "$line" ]; then st=timeout; else st=no-samples; fi
            echo "$ping,$pong,$payload,$r,0,,,,,,,$st" >>"$RAW_CSV"
        fi
        # Multicast-/Socket-Cleanup-Gap zwischen Wiederholungen.
        sleep 1
    done
    # Best-of-N (kleinstes p50) zurueckgeben; leer = alle Runs failed.
    printf '%s' "$best_line"
}

for payload in "${PAYLOADS[@]}"; do
    for ping in "${VENDORS[@]}"; do
        for pong in "${VENDORS[@]}"; do
            idx=$((idx + 1))
            printf '[%3d/%d] payload=%-5d %-8s -> %-8s ' \
                "$idx" "$total" "$payload" "$ping" "$pong"

            line="$(run_cell_repeated "$ping" "$pong" "$payload")"

            if printf '%s' "$line" | grep -q "payload="; then
                n="$(field "$line" n)"
                mn="$(field "$line" min)"
                p50="$(field "$line" p50)"
                p90="$(field "$line" p90)"
                p99="$(field "$line" p99)"
                p999="$(field "$line" p999)"
                mx="$(field "$line" max)"
                status=ok
                printf 'p50_min=%s us (N=%d runs)\n' "$p50" "$REPEAT_RUNS"
            else
                n=0; mn=; p50=; p90=; p99=; p999=; mx=
                status=timeout
                printf 'FAIL (alle %d runs)\n' "$REPEAT_RUNS"
            fi

            echo "$ping,$pong,$payload,$n,$mn,$p50,$p90,$p99,$p999,$mx,$status" >>"$CSV"
        done
    done
done

elapsed=$(( $(date +%s) - started ))
echo
echo "Matrix fertig in $((elapsed / 60)) min $((elapsed % 60)) s."
echo "CSV:    $CSV"

if python3 "$BENCH_DIR/matrix_report.py" "$CSV" >"$REPORT" 2>/dev/null; then
    echo "Report: $REPORT"
else
    echo "Report-Generierung fehlgeschlagen — CSV ist vollstaendig." >&2
fi
