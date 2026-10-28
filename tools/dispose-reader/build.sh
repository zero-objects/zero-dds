#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# build.sh — the Fast DDS, OpenDDS and OpenSplice readers of KeyedPointA
# that the vendor half of crates/zerodds-c-api/tests/writer_dispose_sample_vendors.rs
# and the bridges' dispose tests run against ZeroDDS writers:
#
#   tools/dispose-reader/build.sh [OUT]     (default ${TMPDIR:-/tmp}/dispose-reader)
#
# Prints, per vendor, the ZERODDS_DISPOSE_READER value the test takes, or
# why that vendor was not built. Fast DDS under FASTDDS_HOME with
# FASTDDSGEN; OpenDDS from a built tree under DDS_ROOT (its setenv.sh), as
# the cross-vendor examples use it; OpenSplice 6.9 under OSPL_HOME, run
# through a wrapper that writes a configuration for the requested domain.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
# shellcheck source=../interop-vendors.sh
. "$ROOT/tools/interop-vendors.sh"
out="${1:-${TMPDIR:-/tmp}/dispose-reader}"
mkdir -p "$out"
rc=0

fd="$out/fastdds"
rm -rf "$fd"; mkdir -p "$fd"
cp "$HERE/fastdds/KeyedPointA.idl" "$HERE/fastdds/reader.cpp" "$fd/"
if ( cd "$fd" && "$FASTDDSGEN" -replace KeyedPointA.idl > gen.log 2>&1 \
     && g++ -std=c++17 -O1 -o fastdds_dispose_reader reader.cpp KeyedPointAPubSubTypes.cxx \
            KeyedPointATypeObjectSupport.cxx -I. -I"$FASTDDS_HOME/include" \
            -L"$FASTDDS_HOME/lib" -lfastcdr -lfastdds \
            -Wl,-rpath,"$FASTDDS_HOME/lib" > cc.log 2>&1 ); then
    echo "fastdds: ZERODDS_DISPOSE_READER=$fd/fastdds_dispose_reader"
else
    echo "fastdds: NOT BUILT (see $fd/gen.log, $fd/cc.log)"; rc=1
fi

od="$out/opendds"
rm -rf "$od"; mkdir -p "$od"
if [ -f "${DDS_ROOT:-}/setenv.sh" ]; then
    export LD_LIBRARY_PATH="${LD_LIBRARY_PATH:-}"
    # shellcheck disable=SC1091
    . "$DDS_ROOT/setenv.sh"
    if cmake -S "$HERE/opendds" -B "$od" -DOpenDDS_ROOT="$DDS_ROOT" \
            -DCMAKE_BUILD_TYPE=Release > "$od/cmake.log" 2>&1 \
        && cmake --build "$od" -j > "$od/build.log" 2>&1; then
        echo "opendds: ZERODDS_DISPOSE_READER=$od/opendds_dispose_reader -DCPSConfigFile $HERE/opendds/rtps.ini"
        echo "opendds: LD_LIBRARY_PATH=$LD_LIBRARY_PATH"
    else
        echo "opendds: NOT BUILT (see $od/cmake.log, $od/build.log)"; rc=1
    fi
else
    echo "opendds: NOT PRESENT (DDS_ROOT=${DDS_ROOT:-} has no setenv.sh)"; rc=1
fi
sp="$out/opensplice"
rm -rf "$sp"; mkdir -p "$sp"
if [ -x "$OSPL_HOME/bin/idlpp" ]; then
    cp "$HERE/opensplice/KeyedPointA.idl" "$HERE/opensplice/reader.c" "$sp/"
    # `set +u`: release.com reads LD_LIBRARY_PATH unset.
    # shellcheck disable=SC1091
    if ( set +u; cd "$sp" && . "$OSPL_HOME/release.com" >/dev/null 2>&1 \
         && "$OSPL_HOME/bin/idlpp" -S -l c KeyedPointA.idl > idlpp.log 2>&1 \
         && gcc -O1 -Wall -o reader reader.c KeyedPointASacDcps.c KeyedPointASplDcps.c -I. \
                -I"$OSPL_HOME/include/dcps/C/SAC" -I"$OSPL_HOME/include" \
                -I"$OSPL_HOME/include/sys" -L"$OSPL_HOME/lib" -ldcpssac -lddskernel \
                -Wl,-rpath,"$OSPL_HOME/lib" > cc.log 2>&1 ); then
        # OpenSplice takes the domain from its configuration, not from the
        # participant call, and writes its logs into the working directory:
        # one configuration and one directory per run.
        cat > "$sp/opensplice_dispose_reader" <<WRAPPER
#!/bin/bash
set +u
. "$OSPL_HOME/release.com" >/dev/null 2>&1
cfg=\$(mktemp -d)
trap 'rm -rf "\$cfg"' EXIT
sed "s#<Id>0</Id>#<Id>\$1</Id>#" "$OSPL_HOME/etc/config/ospl_sp_ddsi.xml" > "\$cfg/ospl.xml"
export OSPL_URI="file://\$cfg/ospl.xml"
cd "\$cfg" || exit 1
"$sp/reader" "\$@"; rc=\$?
# OpenSplice reports its own failures to ospl-error.log, not to stderr.
[ \$rc -ne 0 ] && [ -f ospl-error.log ] && cat ospl-error.log >&2
exit \$rc
WRAPPER
        chmod +x "$sp/opensplice_dispose_reader"
        echo "opensplice: ZERODDS_DISPOSE_READER=$sp/opensplice_dispose_reader"
    else
        echo "opensplice: NOT BUILT (see $sp/idlpp.log, $sp/cc.log)"; rc=1
    fi
else
    echo "opensplice: NOT PRESENT (OSPL_HOME=$OSPL_HOME has no bin/idlpp)"; rc=1
fi
exit $rc
