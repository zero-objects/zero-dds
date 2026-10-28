#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Builds the OMG dds-rtps shape_main for OpenSplice CE 6.9 and the wrapper
# the interoperability harness runs.
#
#   tools/opensplice/build-shape-main.sh <dds-rtps-checkout> <opensplice-prefix> <out-dir>
#
# The OMG source is not copied into this repository: patch_shape_main.py
# adds the OPENSPLICE branches to the checkout's srcCxx/shape_main.cxx
# (measured against dds-rtps commit 41bc178117045652b64db3d797a8d9cb06b6e372)
# and refuses to run if a line it expects has changed.
set -euo pipefail
RTPS="${1:?dds-rtps checkout}"
OSPL="${2:?OpenSplice prefix}"
OUT="${3:?output directory}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
mkdir -p "$OUT"
cp "$RTPS/srcCxx/shape_main.cxx" "$HERE/shape_configurator_opensplice.h" "$HERE/shape.idl" "$OUT/"
cd "$OUT"
python3 "$HERE/patch_shape_main.py" shape_main.cxx
set +u
# shellcheck disable=SC1091
source "$OSPL/release.com" >/dev/null
set -u
idlpp -S -l cpp shape.idl
g++ -std=c++11 -O1 -DOPENSPLICE -I. -I"$OSPL/include" -I"$OSPL/include/sys" \
    -I"$OSPL/include/dcps/C++/SACPP" -o shape_main shape_main.cxx shape.cpp \
    shapeDcps.cpp shapeDcps_impl.cpp shapeSplDcps.cpp -L"$OSPL/lib" \
    -ldcpssacpp -lddskernel -Wl,-rpath,"$OSPL/lib"
cat > opensplice_ce-6.9.210323_shape_main_linux <<WRAP
#!/usr/bin/env bash
# OpenSplice CE 6.9 shape_main for the OMG dds-rtps harness: single-process
# DDSI configuration as shipped, no ospl daemon.
#
# OpenSplice creates a participant only in the domain its configuration
# names (<Domain><Id>); ospl_sp_ddsi.xml names 0, and create_participant
# for any other domain fails ("failed to create participant"). The
# domain shape_main is asked for (-d N) gets a copy of the shipped
# configuration with that id, written once per domain beside the binary.
source "$OSPL/release.com" >/dev/null 2>&1
domain=0
args=("\$@")
for ((i = 0; i < \${#args[@]}; i++)); do
    [ "\${args[i]}" = "-d" ] && domain="\${args[i+1]}"
done
config="$OUT/ospl_sp_ddsi_d\${domain}.xml"
if [ ! -f "\$config" ]; then
    sed "s#<Id>0</Id>#<Id>\${domain}</Id>#" "$OSPL/etc/config/ospl_sp_ddsi.xml" > "\$config.\$\$"
    mv "\$config.\$\$" "\$config"
fi
export OSPL_URI=file://\$config
exec "$OUT/shape_main" "\$@"
WRAP
chmod +x opensplice_ce-6.9.210323_shape_main_linux
echo "$OUT/opensplice_ce-6.9.210323_shape_main_linux"
