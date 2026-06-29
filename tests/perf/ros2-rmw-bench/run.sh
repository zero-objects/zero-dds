#!/usr/bin/env bash
# E1 — run latency.py over every available RMW and print a comparison table.
#
# Realizes the iRobot-style "rmw_zerodds runs a realistic ROS-2 graph
# competitively" proof: same rclpy ping<->pong graph, swapped RMW per run.
#
# Prereqs: a sourced ROS 2 (Humble+) with rclpy + the RMW libs to compare. On the
# ZeroDDS bench host that is the micromamba `ros2` env (RoboStack Humble) which
# ships rmw_zerodds_cpp + rmw_cyclonedds_cpp + rmw_fastrtps_cpp.
#
# Usage:  bash run.sh [SAMPLES] [RATE_HZ] [rmw1 rmw2 ...]
set -u
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SAMPLES="${1:-2000}"
RATE="${2:-200}"
shift 2 2>/dev/null || true
RMWS=("$@")
if [ ${#RMWS[@]} -eq 0 ]; then
    RMWS=(rmw_zerodds_cpp rmw_cyclonedds_cpp rmw_fastrtps_cpp)
fi
# Isolate from any ambient ROS graph on the host.
export ROS_DOMAIN_ID="${ROS_DOMAIN_ID:-77}"
export ROS_LOCALHOST_ONLY=1

echo "=== ROS-2 rmw latency matrix (ping<->pong, n=$SAMPLES @ ${RATE}Hz, domain $ROS_DOMAIN_ID) ==="
printf '%-22s %8s %8s %8s %8s\n' "rmw" "n" "p50us" "p90us" "p99us"
for rmw in "${RMWS[@]}"; do
    # Skip an rmw whose lib is not present.
    if ! ls "${CONDA_PREFIX:-/usr}"/lib/lib"${rmw}".so >/dev/null 2>&1 \
        && ! ls /opt/ros/*/lib/lib"${rmw}".so >/dev/null 2>&1; then
        printf '%-22s %8s\n' "$rmw" "MISSING"
        continue
    fi
    sleep 1
    RMW_IMPLEMENTATION="$rmw" python3 "$HERE/latency.py" pong >/tmp/e1_pong.log 2>&1 &
    pong_pid=$!
    sleep 2
    line=$(RMW_IMPLEMENTATION="$rmw" python3 "$HERE/latency.py" ping \
        --samples "$SAMPLES" --rate "$RATE" 2>/tmp/e1_ping_err.log)
    # Stop pong by its tracked PID (NOT pkill -f latency.py — that pattern also
    # matches the caller's own command line and would kill the driver).
    kill "$pong_pid" 2>/dev/null
    wait "$pong_pid" 2>/dev/null
    # Parse the RESULT line.
    n=$(sed -n 's/.* n=\([0-9]*\).*/\1/p' <<<"$line")
    p50=$(sed -n 's/.* p50=\([0-9.]*\).*/\1/p' <<<"$line")
    p90=$(sed -n 's/.* p90=\([0-9.]*\).*/\1/p' <<<"$line")
    p99=$(sed -n 's/.* p99=\([0-9.]*\).*/\1/p' <<<"$line")
    printf '%-22s %8s %8s %8s %8s\n' "$rmw" "${n:-FAIL}" "${p50:--}" "${p90:--}" "${p99:--}"
done
echo "=== done ==="
