#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Builds OpenSplice Community Edition 6.9 from source and installs it.
#
#   tools/opensplice/build-opensplice.sh <source-dir> <prefix> [jdk8-home]
#
# <source-dir>  a clone of github.com/ADLINK-IST/opensplice at tag
#               OSPL_V6_9_210323OSS_RELEASE
# <prefix>      where the HDE tree is copied, e.g. /opt/opensplice-6.9
#
# What a current Linux needs that the project's own CI (GCC 7, Ubuntu 18.04)
# did not, measured on Debian with GCC 14. The sources and the setup files
# stay as released; every change is a make variable on the command line:
#
# * JDK 8: configure refuses javac 21 ("Only Oracle JAVA 1.6 and above").
# * The submodule MPC_ROOT is registered as git://github.com/…, a protocol
#   GitHub no longer serves; the clone is told to fetch it over https. The
#   rewrite goes on the command line (`git -c`), which reaches the clone
#   `submodule update` starts; set in the superproject's own config it does
#   not (git 2.49 and 2.56: the clone went to git:// and hung).
# * CFLAGS_STRICT_UNINITIALIZED: the release config sets
#   -Werror=uninitialized, and GCC 14 reports maybe-uninitialized where
#   GCC 7 did not.
# * -fcommon: GCC 10 made -fno-common the default, and the sources define
#   tentative globals in headers (ld: multiple definition of n_skipped_ifs).
# * CFLAGS_LTO empty: under GCC 14 the LTO plugin reports multiple
#   definitions GCC 7 linked.
set -euo pipefail
SRC="${1:?source directory}"
PREFIX="${2:?install prefix}"
JDK="${3:-/opt/jdk8}"

git -C "$SRC" -c url."https://github.com/".insteadOf=git://github.com/ \
    submodule update --init submods/MPC_ROOT

cd "$SRC"
export JAVA_HOME="$JDK"
export PATH="$JDK/bin:$PATH"
set +u
# shellcheck disable=SC1091
. ./configure x86_64.linux-release
set -u
make -j"$(nproc)" install CFLAGS_LTO= \
    CFLAGS_STRICT_UNINITIALIZED="-fcommon -Wno-error=maybe-uninitialized -Wno-error=incompatible-pointer-types -Wno-error=implicit-function-declaration -Wno-error=int-conversion"

rm -rf "$PREFIX"
cp -a install/HDE/x86_64.linux "$PREFIX"
echo "OpenSplice installed in $PREFIX (source $PREFIX/release.com)"
