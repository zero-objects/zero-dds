# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# The vendor toolchain environment for the interop probes.
#
# This file lives in the repository and not in a home directory. The
# previous copy sat at /root/tc-env.sh on the measurement host, every
# instruction in this tree named it, and a cleanup run removed it — with
# no way to reconstruct it except by reading what the probes expect. A
# tool that is not committed does not survive the machine it lives on.
#
#   . tools/interop-env.sh
#
# Paths were read off the measurement host on 2026-08-13 and each is
# checked below rather than assumed: a missing toolchain leaves its
# variables unset and prints one line, so a probe that then fails says
# which vendor is absent instead of failing on a path that never existed.

_zd_env_have() { [ -e "$1" ]; }
_zd_env_missing=""

# Rust. The host's cargo is a rustup install under root's home.
if _zd_env_have "$HOME/.cargo/env"; then
    # shellcheck disable=SC1091
    . "$HOME/.cargo/env"
else
    _zd_env_missing="$_zd_env_missing cargo"
fi

# Eclipse Cyclone DDS — idlc, headers and libraries.
if _zd_env_have /opt/cyclone/bin/idlc; then
    export CYCLONE_HOME=/opt/cyclone
else
    _zd_env_missing="$_zd_env_missing cyclone"
fi

# eProsima Fast DDS, and fastddsgen from the source checkout.
if _zd_env_have /opt/fastdds/include; then
    export FASTDDS_HOME=/opt/fastdds
else
    _zd_env_missing="$_zd_env_missing fastdds"
fi
if _zd_env_have /root/vendors/fastddsgen/scripts/fastddsgen; then
    export FASTDDSGEN=/root/vendors/fastddsgen/scripts/fastddsgen
else
    _zd_env_missing="$_zd_env_missing fastddsgen"
fi

# OpenDDS ships its own environment script; it sets DDS_ROOT, ACE_ROOT,
# TAO_ROOT and the library path together, which is also where tao_idl
# comes from on this host — there is no separate ACE_wrappers install.
#
# There are two OpenDDS trees on the measurement host and the order below
# is not cosmetic. `opendds-secure` is built with DDS-Security; `opendds`
# is not, and its `libOpenDDS_Dcps.so` defines none of the four
# `get_crypto_handle` symbols the other one exports. Every OpenDDS binary
# on the host — the shapes examples under /root/xv-demos included — is
# linked against `opendds-secure` by RPATH, and an `LD_LIBRARY_PATH`
# naming `opendds` overrides that RPATH: the binaries then die at startup
# with `symbol lookup error: undefined symbol:
# _ZNK7OpenDDS4DCPS14DataReaderImpl17get_crypto_handleEv`. Sourcing the
# secure tree first costs nothing (it runs unsecured setups unchanged)
# and keeps that failure out of the probes.
if _zd_env_have /root/vendors/opendds-secure/setenv.sh; then
    # shellcheck disable=SC1091
    . /root/vendors/opendds-secure/setenv.sh
elif _zd_env_have /root/vendors/opendds/setenv.sh; then
    # shellcheck disable=SC1091
    . /root/vendors/opendds/setenv.sh
else
    _zd_env_missing="$_zd_env_missing opendds"
fi

# RTI Connext. NDDSHOME is what rtiddsgen and the probe's linker read;
# the architecture string is part of the library path and is checked
# because an RTI upgrade renames it — 7.3.0 to 8.5.0 has bitten this
# tree once already.
if _zd_env_have /opt/rti.com/rti_connext_dds-7.7.0/lib/x64Linux4gcc8.5.0; then
    export NDDSHOME=/opt/rti.com/rti_connext_dds-7.7.0
    export NDDSARCH=x64Linux4gcc8.5.0
else
    _zd_env_missing="$_zd_env_missing rti"
fi

# JacORB — the Java ORB column.
if _zd_env_have /opt/jacorb/lib; then
    export JACORB_HOME=/opt/jacorb
else
    _zd_env_missing="$_zd_env_missing jacorb"
fi

# omniORB comes from the distribution's packages; omniidl is on PATH and
# the headers are under /usr/include/omniORB4. Nothing to export, but its
# absence is still worth naming.
_zd_env_have /usr/bin/omniidl || _zd_env_missing="$_zd_env_missing omniorb"

if [ -n "$_zd_env_missing" ]; then
    echo "interop-env: not installed here:$_zd_env_missing" >&2
fi

unset -f _zd_env_have
unset _zd_env_missing
