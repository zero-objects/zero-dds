#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# A whole RTPS session of one implementation, captured on the loopback.
#
#   tools/wire-capture/capture-rtps-session.sh --impl cyclone \
#       --out tools/wire-capture/rtps-sessions
#
#   --impl <name>   cyclone | rti | fastdds | opendds
#   --domain <n>    domain id; default is one per implementation so two
#                   runs at once do not discover each other
#   --seconds <n>   how long the two participants live; default 25
#   --out <dir>     <dir>/<impl>/session.frames and the log beside it
#   --keep-pcap     keep the capture next to the frames
#
# ## Why this is not `wire-golden.sh`
#
# `wire-golden.sh` asks what a *serialiser* put in a sample, so it keeps
# the user `DATA` packets of a run and throws the rest away. This asks
# what a *protocol stack* put on the socket, so it keeps everything: the
# SPDP announcement a participant repeats on a timer, the SEDP
# publication and subscription announcements, the `HEARTBEAT` a reliable
# writer sends, the `ACKNACK` its reader answers with, and the user data
# among them.
#
# ## What a run is
#
# One implementation, one domain of its own, a publisher and a
# subscriber from that implementation's own shipped tooling — nothing in
# the capture was written by this tree. `tcpdump` records UDP on every
# interface, is started before the participants and stopped after them,
# and `extract-rtps-datagrams.py` keeps the datagrams whose RTPS header
# carries this implementation's vendor id, one hex line each.
#
# A run that produced no SPDP is a run whose participants never came up,
# and the script says so and writes nothing: an empty frames file passes
# every assertion that is not run against it.
#
# The capture is on `any` and not on `lo`. Two participants on one host
# do not necessarily talk over the loopback: SPDP is multicast and goes
# out of whichever interface the stack selected, and a first run on `lo`
# recorded zero packets for a Cyclone pair that had exchanged 2100
# samples. `any` sees both, at the price of seeing a packet once per
# interface it crosses, which the extractor collapses.
set -uo pipefail

TOOLS="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

impl=""
domain=""
seconds=25
out=""
keep_pcap=0

while [ $# -gt 0 ]; do
    case "$1" in
        --impl) impl="$2"; shift 2 ;;
        --domain) domain="$2"; shift 2 ;;
        --seconds) seconds="$2"; shift 2 ;;
        --out) out="$2"; shift 2 ;;
        --keep-pcap) keep_pcap=1; shift ;;
        --help|-h) sed -n '4,16p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *) echo "capture-rtps-session: \`$1\` is not an option" >&2; exit 2 ;;
    esac
done
[ -n "$impl" ] || { echo "capture-rtps-session: --impl is required" >&2; exit 2; }
[ -n "$out" ] || { echo "capture-rtps-session: --out is required" >&2; exit 2; }

# One domain per implementation. Two implementations on one domain would
# discover each other and each capture would hold the other's octets
# under this one's name.
if [ -z "$domain" ]; then
    case "$impl" in
        cyclone) domain=61 ;;
        rti)     domain=62 ;;
        fastdds) domain=63 ;;
        # 42 and not a domain of this script's choosing: the OpenDDS
        # DevGuide `Messenger` writes `create_participant(42, …)` into
        # `Publisher.cpp` and takes no domain argument, so 42 is the
        # domain the run is on whatever this said.
        opendds) domain=42 ;;
        *) echo "capture-rtps-session: no default domain for \`$impl\`" >&2; exit 2 ;;
    esac
fi

# The RTPS header's vendor id, per implementation. Attribution is read
# off the message and not off the port: §9.6.1.1's default port mapping
# is a default and an implementation that moved off it would vanish from
# a port-filtered capture without saying so. A first run filtered
# `udp portrange 17650-17899` on domain 41 and caught four SPDP packets
# of a Cyclone pair that had exchanged 2100 samples.
case "$impl" in
    cyclone) vendor_id=0110 ;;
    rti)     vendor_id=0101 ;;
    fastdds) vendor_id=010f ;;
    opendds) vendor_id=0103 ;;
esac

# How many participants of that vendor the run puts on the domain. Two
# for a publisher and a subscriber; three for the ROS 2 pair, because
# `ros2` starts a `_ros2daemon` participant of its own on the same
# `ROS_DOMAIN_ID` and that daemon is a Fast DDS participant like the
# other two.
expect_participants=2
[ "$impl" = fastdds ] && expect_participants=3

work="$out/$impl"
mkdir -p "$work"
log="$work/session.log"
pcap="$work/session.pcap"
frames="$work/session.frames"
: > "$log"

say() { echo "$@" | tee -a "$log" >&2; }

say "capture-rtps-session: impl=$impl domain=$domain vendor-id=$vendor_id"

# Every participant is started with `setsid` and stopped by its process
# group. Three of the four launchers are wrapper scripts — `rtiddsping`
# is `/bin/sh` around the real binary and `ros2` is a Python entry point
# that forks — and killing the wrapper leaves the participant on the
# domain, where the next run discovers it and records its octets under
# the next implementation's name.
groups=()
run_detached() {
    local tag=$1; shift
    local file="$work/$tag.pgid"
    rm -f "$file"
    # The process writes its own pid into `$file` and only then execs, so
    # what is killed later is the group leader the participant actually
    # runs under. `$!` is not that: three of the four launchers are
    # wrappers — `rtiddsping` is `/bin/sh` around the real binary,
    # `ros2` a Python entry point that forks — and killing the wrapper
    # leaves the participant on the domain, where the next run discovers
    # it and records its octets under the next implementation's name.
    setsid bash -c 'echo $$ > "$1"; shift; exec "$@"' _ "$file" "$@" \
        >> "$log" 2>&1 &
    groups+=("$file")
    local waited=0
    while [ ! -s "$file" ] && [ "$waited" -lt 50 ]; do
        sleep 0.1
        waited=$((waited + 1))
    done
}
cleanup() {
    local file pgid signal
    for signal in TERM KILL; do
        for file in "${groups[@]:-}"; do
            [ -s "${file:-/nonexistent}" ] || continue
            pgid=$(cat "$file")
            # Never signal group 0 — that is this script's own group.
            case "$pgid" in ''|0|1) continue ;; esac
            kill "-$signal" -- "-$pgid" 2>/dev/null
        done
        [ "$signal" = TERM ] && sleep 2
    done
    [ -n "${tcpdump_pid:-}" ] && kill "$tcpdump_pid" 2>/dev/null
    wait 2>/dev/null
}
trap cleanup EXIT

tcpdump -i any -s 0 -U -w "$pcap" udp >> "$log" 2>&1 &
tcpdump_pid=$!
sleep 2
kill -0 "$tcpdump_pid" 2>/dev/null || { say "tcpdump did not start"; exit 1; }

case "$impl" in
    cyclone)
        home="${CYCLONE_HOME:-/opt/cyclone}"
        run_detached sub env LD_LIBRARY_PATH="$home/lib" "$home/bin/ddsperf" \
            -i "$domain" -D "$seconds" sub
        sleep 3
        # 10 Hz and not ddsperf's default: what this capture is for is
        # the variety of submessages, and a faster writer only repeats
        # the DATA/HEARTBEAT pair. A 100 Hz run recorded 2138 DATA and
        # a 452 KB frames file for the same 44 discovery announcements.
        run_detached pub env LD_LIBRARY_PATH="$home/lib" "$home/bin/ddsperf" \
            -i "$domain" -D "$((seconds - 4))" pub 10Hz
        ;;
    rti)
        home="${NDDSHOME:?NDDSHOME is unset — . tools/interop-env.sh}"
        # `-timeout` is rtiddsping's discovery wait and not a run
        # length; the run ends when `cleanup` stops the process group.
        run_detached sub "$home/bin/rtiddsping" -domainId "$domain" -subscriber
        sleep 3
        run_detached pub "$home/bin/rtiddsping" -domainId "$domain" -publisher
        ;;
    fastdds)
        # Fast DDS ships no publisher of its own outside its examples;
        # the ROS 2 install on the measurement host carries the same
        # library under `rmw_fastrtps_cpp`, and a `ros2 topic` pair is a
        # real Fast DDS participant with real endpoints.
        prefix="${ROS_PREFIX:-/root/micromamba/envs/ros2}"
        # `set -u` off across the sourcing: ROS 2's `setup.bash` reads
        # `AMENT_TRACE_SETUP_FILES` before setting it, and under `-u`
        # that ends the script with the participants never started.
        set +u
        # shellcheck disable=SC1091
        . "$prefix/setup.bash" >> "$log" 2>&1
        set -u
        export RMW_IMPLEMENTATION=rmw_fastrtps_cpp ROS_DOMAIN_ID="$domain"
        run_detached sub "$prefix/bin/ros2" topic echo /zdsession std_msgs/msg/String
        sleep 5
        run_detached pub "$prefix/bin/ros2" topic pub -r 10 /zdsession \
            std_msgs/msg/String '{data: zerodds-session}'
        ;;
    opendds)
        home="${OPENDDS_HOME:-/opt/opendds}"
        examples="${OPENDDS_MESSENGER:-/root/vendors/opendds/DevGuideExamples/DCPS/Messenger}"
        [ -x "$examples/publisher" ] || { say "no OpenDDS Messenger at $examples"; exit 1; }
        cat > "$work/rtps.ini" <<INI
[common]
DCPSGlobalTransportConfig=\$file
DCPSDefaultDiscovery=DEFAULT_RTPS

[rtps_discovery/default]
ResendPeriod=2

[transport/the_rtps_transport]
transport_type=rtps_udp
INI
        cd "$work" || exit 1
        run_detached sub env LD_LIBRARY_PATH="$home/lib" "$examples/subscriber" \
            -DCPSConfigFile rtps.ini -DCPSDebugLevel 0 -DCPSBit 1
        sleep 3
        run_detached pub env LD_LIBRARY_PATH="$home/lib" "$examples/publisher" \
            -DCPSConfigFile rtps.ini -DCPSDebugLevel 0 -DCPSBit 1
        cd - > /dev/null || exit 1
        ;;
    *)
        say "capture-rtps-session: \`$impl\` is not an implementation"
        exit 2
        ;;
esac

sleep "$seconds"
cleanup
trap - EXIT
sleep 1

python3 "$TOOLS/extract-rtps-datagrams.py" "$pcap" --out "$frames" \
    --vendor-id "$vendor_id" --domain "$domain" --summary | tee -a "$log"
status=${PIPESTATUS[0]}
[ "$status" -eq 0 ] || { say "extraction failed"; exit 1; }

lines=$(wc -l < "$frames")
spdp=$(grep -c 'builtin-writer	SPDP participant' "$log")
if [ "$lines" -eq 0 ] || [ "$spdp" -eq 0 ]; then
    say "no SPDP in $lines datagrams — the participants did not come up"
    rm -f "$frames"
    exit 1
fi

# Exactly the participants this run started, counted by the sender GUID
# prefix in the RTPS header. One more is another run of the same vendor
# on the same host, and its octets would be recorded here under this
# run's name — a contaminated capture that every assertion passes. It
# has happened on the measurement host: a second harness ran
# `rtiddsping` on the same domain while this one did.
participants=$(awk -F'\t' '$1 == "participants" { print $2 }' "$log")
if [ "${participants:-0}" -ne "$expect_participants" ]; then
    say "$participants participants of vendor $vendor_id on domain $domain, \
expected the $expect_participants this run started — another run is on the \
same domain"
    rm -f "$frames"
    exit 1
fi
say "wrote $lines datagrams from $participants participants to $frames"

[ "$keep_pcap" -eq 1 ] || rm -f "$pcap"
