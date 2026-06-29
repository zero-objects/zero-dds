#!/usr/bin/env bash
# atomic.sh — 1 ping × 1 pong × 1 payload = 1 CSV-Zeile.
#
# Garantiert frischer Process-Tree: dieser Skript-Process wird vom
# Wrapper als fresh `bash atomic.sh ...` aufgerufen. KEIN state aus
# vorigen Runs, KEIN loop, KEIN shared shell-zustand.
#
# Args:  <ping_vendor> <pong_vendor> <payload_bytes>
# Env:   ATOMIC_SETUP_DIR (default: same-dir/setup)
#        ATOMIC_BUILD_DIR (default: ../build)
#        ATOMIC_SAMPLES   (default: 2000)
#        ATOMIC_WARMUP    (default: 200)
#        ATOMIC_DOMAIN    (default: 200)
#        ATOMIC_PING_TIMEOUT (default: 60)
#        ATOMIC_POSTHOOK  (default: unset; wenn gesetzt, source'd nach setup)
#        HOST_TAG         (z.B. "arm-host" oder "x86-host") — fuer setup-Datei-Auswahl
#
# Stdout: einzige CSV-Datenzeile (kein header):
#   ping,pong,payload,domain,n,min,p50,p90,p99,p999,max
# Exit:   0 wenn ping-Output geparsed, 1 sonst (CSV-Zeile geht trotzdem
#         mit status-spalte raus).
#
# Beispiel:
#   HOST_TAG=arm-host bash atomic.sh zerodds zerodds 0
#   → zerodds,zerodds,0,200,2000,26.5,38.3,...,ok
set -u

PING=${1:?usage: atomic.sh <ping> <pong> <payload>}
PONG=${2:?usage: atomic.sh <ping> <pong> <payload>}
PAYLOAD=${3:?usage: atomic.sh <ping> <pong> <payload>}

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
: "${ATOMIC_SETUP_DIR:=$SCRIPT_DIR/setup}"
: "${ATOMIC_BUILD_DIR:=$SCRIPT_DIR/../build}"
: "${ATOMIC_SAMPLES:=2000}"
: "${ATOMIC_WARMUP:=200}"
: "${ATOMIC_DOMAIN:=200}"
: "${ATOMIC_PING_TIMEOUT:=60}"
: "${HOST_TAG:=$(uname -s | tr '[:upper:]' '[:lower:]')}"

# macOS hat `timeout` nicht in PATH; coreutils-Variante heisst gtimeout.
# Fallback: kein timeout-binary verfuegbar -> bg-kill mit sleep&kill.
TIMEOUT_BIN=""
if command -v timeout >/dev/null 2>&1; then
    TIMEOUT_BIN="timeout"
elif command -v gtimeout >/dev/null 2>&1; then
    TIMEOUT_BIN="gtimeout"
elif command -v /opt/homebrew/bin/gtimeout >/dev/null 2>&1; then
    TIMEOUT_BIN="/opt/homebrew/bin/gtimeout"
fi
if [ -z "$TIMEOUT_BIN" ]; then
    echo "atomic.sh: no timeout/gtimeout in PATH (brew install coreutils)" >&2
    exit 1
fi

# Setup laden: pro vendor genau eine env-Datei pro host. Wenn nicht
# vorhanden → fail klar, kein silent fallback.
load_env() {
    local vendor=$1
    local env_file="$ATOMIC_SETUP_DIR/${HOST_TAG}-${vendor}.env"
    if [ ! -f "$env_file" ]; then
        echo "atomic.sh: missing $env_file" >&2
        return 1
    fi
    # shellcheck disable=SC1090
    . "$env_file"
}

# Lade beide setups — pong UND ping (selber process, ueberlappende env
# vars sind ok solange beide vendoren konsistent sind, z.B. NDDSHOME).
load_env "$PONG" || exit 1
load_env "$PING" || exit 1

# Optional: caller-injected post-hook (z.B. PATH-erweiterung).
if [ -n "${ATOMIC_POSTHOOK:-}" ] && [ -f "$ATOMIC_POSTHOOK" ]; then
    # shellcheck disable=SC1090
    . "$ATOMIC_POSTHOOK"
fi

export ZERODDS_BENCH_DOMAIN="$ATOMIC_DOMAIN"
# Transport-selection: UDPv4 (default), UDPv6, SHM, TCPv4.
# Apps reagieren via ZERODDS_BENCH_TRANSPORT env-var.
: "${ZERODDS_BENCH_TRANSPORT:=UDPv4}"
export ZERODDS_BENCH_TRANSPORT

cd "$ATOMIC_BUILD_DIR" || { echo "atomic.sh: build dir not found: $ATOMIC_BUILD_DIR" >&2; exit 1; }

PING_BIN="./${PING}-roundtrip"
PONG_BIN="./${PONG}-roundtrip"
[ -x "$PING_BIN" ] || { echo "atomic.sh: missing $PING_BIN" >&2; exit 1; }
[ -x "$PONG_BIN" ] || { echo "atomic.sh: missing $PONG_BIN" >&2; exit 1; }

# Vendor-spezifische CLI-args (OpenDDS braucht -DCPSConfigFile).
# Pro Transport eine .ini: rtps_udp (default), shm, tcp.
case "$ZERODDS_BENCH_TRANSPORT" in
    SHM)   OPENDDS_INI="opendds_rtps_shm.ini" ;;
    TCPv4) OPENDDS_INI="opendds_rtps_tcp.ini" ;;
    *)     OPENDDS_INI="opendds_rtps.ini"     ;;
esac
PONG_ARGS=""
PING_ARGS=""
if [ "$PONG" = "opendds" ]; then
    PONG_ARGS="-DCPSConfigFile $SCRIPT_DIR/../$OPENDDS_INI"
fi
if [ "$PING" = "opendds" ]; then
    PING_ARGS="-DCPSConfigFile $SCRIPT_DIR/../$OPENDDS_INI"
fi

# Vendor-spezifische pong-startup-delays (Discovery-Setup-Zeit).
case "$PONG" in
    fastdds) POND_SLEEP=6 ;;
    opendds) POND_SLEEP=8 ;;
    rti)     POND_SLEEP=5 ;;
    *)       POND_SLEEP=3 ;;
esac

# Logs in tmpdir damit dieser run vollstaendig isoliert ist.
TMP=$(mktemp -d -t atomic.XXXXXX)
trap 'rm -rf "$TMP"' EXIT

PONG_LOG="$TMP/pong.log"
PING_LOG="$TMP/ping.log"

# Start pong als bg-child dieses scripts. Stirbt mit dem script via trap.
# shellcheck disable=SC2086
$PONG_BIN pong 60 $PONG_ARGS >"$PONG_LOG" 2>&1 &
PONG_PID=$!

sleep "$POND_SLEEP"

# Run ping in foreground mit timeout. shellcheck disable=SC2086
"$TIMEOUT_BIN" "$ATOMIC_PING_TIMEOUT" $PING_BIN ping \
    --payload "$PAYLOAD" --samples "$ATOMIC_SAMPLES" --warmup "$ATOMIC_WARMUP" \
    $PING_ARGS >"$PING_LOG" 2>&1
PING_RC=$?

# Pong gracefully beenden.
kill -TERM "$PONG_PID" 2>/dev/null
sleep 0.5
kill -KILL "$PONG_PID" 2>/dev/null
wait "$PONG_PID" 2>/dev/null

LINE=$(grep "^payload=" "$PING_LOG" | head -1)

if [ -z "$LINE" ]; then
    # Kein gültiger Output — Fehler-CSV-Zeile + stderr-Hint.
    echo "$PING,$PONG,$PAYLOAD,$ATOMIC_DOMAIN,0,,,,,,,timeout(rc=$PING_RC)"
    {
        echo "atomic.sh: no 'payload=...' line in ping output (rc=$PING_RC)"
        echo "--- ping-log tail ---"
        tail -10 "$PING_LOG"
        echo "--- pong-log tail ---"
        tail -10 "$PONG_LOG"
    } >&2
    exit 1
fi

# Parse + emit CSV. grep -oE auf der line.
extract() { printf '%s' "$LINE" | grep -oE "$1=[0-9.]+" | head -1 | cut -d= -f2; }
N=$(extract n)
MIN=$(extract min)
P50=$(extract p50)
P90=$(extract p90)
P99=$(extract p99)
P999=$(extract p999)
MAX=$(extract max)

echo "$PING,$PONG,$PAYLOAD,$ATOMIC_DOMAIN,$N,$MIN,$P50,$P90,$P99,$P999,$MAX,ok"
