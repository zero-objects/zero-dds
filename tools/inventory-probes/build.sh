#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# build.sh — the build recipes the newer inventory probes share.
#
# `run.sh`'s recipes are C-only and predate the OpenDDS and Fast DDS C++
# probes of `longdouble-xcdr2` and `encapsulation-be`, which need
# `tao_idl` beside `opendds_idl` and the C++ generated code. They are
# here rather than duplicated into each group's driver, and they are the
# same commands `tools/wireprobe/wireprobe-{fastdds,opendds}` run — a
# probe built differently from the harness would be measuring the build.
set -uo pipefail

build_cpp_fastdds() { # <dir> <idl> <source>
    local d=$1 idl=$2 src=$3 stem
    stem="$(basename "${idl%.idl}")"
    ( cd "$d" && "$FASTDDSGEN" -replace "$(basename "$idl")" > gen.log 2>&1 \
      && g++ -std=c++17 -o probe "$(basename "$src")" "${stem}PubSubTypes.cxx" \
             "${stem}TypeObjectSupport.cxx" -I. -I"$FASTDDS_HOME/include" \
             -L"$FASTDDS_HOME/lib" -lfastcdr -lfastdds \
             -Wl,-rpath,"$FASTDDS_HOME/lib" > cc.log 2>&1 ) \
      || { echo "  the Fast DDS probe did not build" >&2
           grep -m4 "error" "$d"/*.log >&2; return 1; }
}

build_cpp_opendds() { # <dir> <idl> <source>
    local d=$1 idl=$2 src=$3
    ( cd "$d" \
      && "$OPENDDS_HOME/bin/tao_idl" -I"$OPENDDS_HOME/include" -Sa -St \
             --idl-version 4 "$(basename "$idl")" > tao.log 2>&1 \
      && "$OPENDDS_HOME/bin/opendds_idl" -I"$OPENDDS_HOME/include" \
             --idl-version 4 "$(basename "$idl")" > idl.log 2>&1 \
      && "$OPENDDS_HOME/bin/tao_idl" -I"$OPENDDS_HOME/include" -Sa -St \
             --idl-version 4 "$(basename "${idl%.idl}")TypeSupport.idl" >> tao.log 2>&1 \
      && g++ -std=c++17 -o probe "$(basename "$src")" \
             "$(basename "${idl%.idl}")C.cpp" \
             "$(basename "${idl%.idl}")TypeSupportImpl.cpp" \
             -I. -I"$OPENDDS_HOME/include" -L"$OPENDDS_HOME/lib" \
             -lOpenDDS_Dcps -lTAO_AnyTypeCode -lTAO_Valuetype -lTAO -lACE \
             -Wl,-rpath,"$OPENDDS_HOME/lib" > cc.log 2>&1 ) \
      || { echo "  the OpenDDS probe did not build" >&2
           grep -m4 -i "error" "$d"/*.log >&2; return 1; }
}

build_c_rti() { # <dir> <idl> <source>
    local d=$1 idl=$2 src=$3 stem
    stem="$(basename "${idl%.idl}")"
    ( cd "$d" && "$NDDSHOME/bin/rtiddsgen" -language C -platform "$RTI_TARGET" \
             -replace "$(basename "$idl")" > gen.log 2>&1 \
      && gcc -o probe "$(basename "$src")" "$stem.c" "${stem}Plugin.c" "${stem}Support.c" \
             -I. -I"$NDDSHOME/include" -I"$NDDSHOME/include/ndds" -L"$RTI_LIB" \
             -lnddsc -lnddscore -ldl -lm -lpthread -lrt -Wl,-rpath,"$RTI_LIB" \
             -DRTI_UNIX -DRTI_LINUX -DRTI_64BIT > cc.log 2>&1 ) \
      || { echo "  the RTI probe did not build" >&2
           grep -m4 -i error "$d"/*.log >&2; return 1; }
}
