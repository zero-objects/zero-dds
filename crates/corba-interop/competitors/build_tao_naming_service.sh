#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Builds the TAO Naming_Service (tao_cosnaming) from the ACE+TAO orbsvcs source so
# that the cross-ORB direction "ZeroDDS client → TAO naming daemon" can run in
# run_interop.sh. The OpenDDS-bundled TAO under /opt/opendds-secure is built
# minimally (TAO core WITHOUT orbsvcs); Debian has no TAO package. Hence: fetch
# the orbsvcs source and build ONLY Svc_Utils + CosNaming(+_Skel/_Serv) +
# Naming_Service, linked against the installed TAO core libs (no core rebuild).
#
# Idempotent: skips download/setup if already present. Sets
# TAO_NAMING_BIN for run_interop.sh (the default path is BIN below).
set -euo pipefail

TAO_PREFIX=/opt/opendds-secure              # installed ACE/TAO 6.5.24 core libs + tao_idl
BUILD=/root/build/acetao
ACE="$BUILD/ACE_wrappers"
BIN="$ACE/TAO/orbsvcs/Naming_Service/tao_cosnaming"
VER=6.5.24                                   # must match the installed libACE/libTAO
URL="https://github.com/DOCGroup/ACE_TAO/releases/download/ACE%2BTAO-6_5_${VER##*.}/ACE+TAO-src-${VER}.tar.bz2"

[ -x "$BIN" ] && { echo "tao_cosnaming already built: $BIN"; echo "TAO_NAMING_BIN=$BIN"; exit 0; }
[ -x "$TAO_PREFIX/bin/tao_idl" ] || { echo "ERROR: $TAO_PREFIX/bin/tao_idl missing (TAO core not installed)"; exit 1; }

mkdir -p "$BUILD"; cd "$BUILD"
if [ ! -d "$ACE" ]; then
  echo "== Fetching ACE+TAO $VER source =="
  curl -sL -o src.tar.bz2 "$URL"
  tar xjf src.tar.bz2
fi

export ACE_ROOT="$ACE" TAO_ROOT="$ACE/TAO" MPC_ROOT="$ACE/MPC" DANCE_ROOT=/tmp/none CIAO_ROOT=/tmp/none
export PATH="$TAO_PREFIX/bin:$ACE/bin:$PATH" LD_LIBRARY_PATH="$TAO_PREFIX/lib:$ACE/lib"

# Config + platform from the install (ABI-compatible) + install include (for the
# generated TAO core headers like orb_typesC.h, which only live in the install).
echo '#include "ace/config-linux.h"' > "$ACE/ace/config.h"
cp "$TAO_PREFIX/share/ace/include/makeinclude/platform_macros.GNU" "$ACE/include/makeinclude/platform_macros.GNU"
grep -q "opendds-secure/include" "$ACE/include/makeinclude/platform_macros.GNU" \
  || echo "CCFLAGS += -I$TAO_PREFIX/include" >> "$ACE/include/makeinclude/platform_macros.GNU"

# Mirror the installed core libs + tao_idl into the source tree (no core rebuild).
mkdir -p "$ACE/lib" "$ACE/bin"
ln -sf "$TAO_PREFIX"/lib/lib*.so* "$ACE/lib/" 2>/dev/null || true
ln -sf "$TAO_PREFIX/bin/tao_idl" "$ACE/bin/tao_idl"
ln -sf "$TAO_PREFIX/bin/ace_gperf" "$ACE/bin/ace_gperf" 2>/dev/null || true

# Minimal workspace: only the naming chain.
cd "$TAO_ROOT/orbsvcs"
cat > naming.mwc <<EOF
workspace {
  orbsvcs/Svc_Utils.mpc
  orbsvcs/CosNaming.mpc
  orbsvcs/CosNaming_Skel.mpc
  orbsvcs/CosNaming_Serv.mpc
  Naming_Service/Naming_Service.mpc
}
EOF
"$ACE/bin/mwc.pl" -type gnuace naming.mwc
make -j"$(nproc)"

[ -x "$BIN" ] && echo "OK: $BIN" || { echo "ERROR: tao_cosnaming not built"; exit 1; }
echo "For run_interop.sh: export TAO_NAMING_BIN=$BIN  (or it is the default path)"
