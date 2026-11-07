#!/usr/bin/env bash
# Cyclone DDS at a pinned commit, built from source, and the two OMG suite
# clients the suites' own CMake projects build against it: the dds-rtps
# `shape_main` (srcC/cyclone-dds-cmake) and the dds-xtypes `test_main`
# (src/c/cyclone-dds-cmake), named `<product>_shape_main_linux` and
# `<product>_test_main_linux`, the names the harnesses read a product from.
#
#   build-cyclone-suite-clients.sh <commit> <dds-rtps dir> <dds-xtypes dir> <work dir> <out dir> <product>
#
# For Cyclone `master` beside the released 11.0.1 (GitHub #60), at the commit of
# pins.tsv: ci/Dockerfile.omg runs it in a build stage of the ci-omg image,
# provision.sh --cyclone-master in the interop workflow of GitHub; the Linux test
# host runs the same script. The executables link Cyclone statically (BUILD_SHARED_LIBS=OFF,
# no OpenSSL, no DDS Security), as the suites' released Cyclone executables
# do (they need libc only), so the ci-omg image carries no Cyclone library.
# <work dir>/host keeps the shared build of the same commit (idlc, libddsc,
# the headers).
#
# Both clients link Cyclone as its default build makes it, with the type
# library (ENABLE_TYPELIB, ENABLE_TYPE_DISCOVERY and ENABLE_TOPIC_DISCOVERY
# on): what an application built against this commit announces. The released
# 11.0.1 `shape_main` of vendors.tsv links none of the type library
# and its DATA(r) carries no PID_TYPE_INFORMATION; that is the vendor's
# release and stays as it is.
set -euo pipefail
commit=$1 rtps=$2 xtypes=$3 work=$4 out=$5 product=$6
jobs=${JOBS:-$(nproc)}
mkdir -p "$work" "$out"
work=$(cd "$work" && pwd)
src=$work/cyclonedds
prefix=$work/install

if [ ! -d "$src/.git" ]; then
    git init -q "$src"
    git -C "$src" remote add origin https://github.com/eclipse-cyclonedds/cyclonedds.git
fi
git -C "$src" fetch -q --depth 1 origin "$commit"
git -C "$src" checkout -q --force FETCH_HEAD
test "$(git -C "$src" rev-parse HEAD)" = "$commit" \
    || { echo "cyclonedds: $(git -C "$src" rev-parse HEAD) is not $commit" >&2; exit 1; }

# Two builds, as Cyclone's CMakeLists.txt advises for a static library:
# a shared one for the host (idlc, and what the matrix column of this
# commit links), and the static one built as if cross-compiling against it.
cmake -S "$src" -B "$work/build-host" -DCMAKE_BUILD_TYPE=RelWithDebInfo \
    -DCMAKE_INSTALL_PREFIX="$work/host" -DBUILD_SHARED_LIBS=ON \
    -DENABLE_SSL=OFF -DENABLE_SECURITY=OFF -DENABLE_ICEORYX=OFF -DENABLE_ICEORYX2=OFF \
    -DBUILD_IDLC=ON -DBUILD_DDSPERF=OFF \
    -DBUILD_TESTING=OFF -DBUILD_EXAMPLES=OFF >"$work/cyclonedds-host-configure.log"
cmake --build "$work/build-host" --target install -j"$jobs" >"$work/cyclonedds-host-build.log"
cmake -S "$src" -B "$work/build" -DCMAKE_BUILD_TYPE=RelWithDebInfo \
    -DCMAKE_CROSSCOMPILING=1 -DCMAKE_SYSTEM_NAME=Linux -DCMAKE_PREFIX_PATH="$work/host" \
    -DCMAKE_INSTALL_PREFIX="$prefix" -DBUILD_SHARED_LIBS=OFF \
    -DENABLE_SSL=OFF -DENABLE_SECURITY=OFF -DENABLE_ICEORYX=OFF -DENABLE_ICEORYX2=OFF \
    -DBUILD_DDSPERF=OFF \
    -DBUILD_TESTING=OFF -DBUILD_EXAMPLES=OFF >"$work/cyclonedds-configure.log"
cmake --build "$work/build" --target install -j"$jobs" >"$work/cyclonedds-build.log"

# The suites' sources are copied: the dds-rtps project runs idlc into the
# directory above its CMake project. GCC 14 (Debian trixie) refuses
# shape_main.c's `dds_qset_partition(qos, 1, &opts->partition)` (char *const *
# for const char **) as an error unless told it is a warning; the sources are
# not changed.
cflags="-Wno-error=incompatible-pointer-types"
rm -rf "$work/rtps-src" "$work/xtypes-src" "$work/rtps-build" "$work/xtypes-build"
cp -r "$rtps/srcC" "$work/rtps-src"
cp -r "$xtypes/src/c" "$work/xtypes-src"
cmake -S "$work/rtps-src/cyclone-dds-cmake" -B "$work/rtps-build" -DCMAKE_BUILD_TYPE=RelWithDebInfo \
    -DBUILD_SHARED_LIBS=OFF -DIDLC_ROOT="$work/host/bin" -DCMAKE_PREFIX_PATH="$prefix" \
    -DCMAKE_C_FLAGS="$cflags" >/dev/null
cmake --build "$work/rtps-build" -j"$jobs" >/dev/null
cmake -S "$work/xtypes-src/cyclone-dds-cmake" -B "$work/xtypes-build" -DCMAKE_BUILD_TYPE=RelWithDebInfo \
    -DBUILD_SHARED_LIBS=OFF -DCMAKE_PREFIX_PATH="$prefix" -DCMAKE_C_FLAGS="$cflags" >/dev/null
cmake --build "$work/xtypes-build" -j"$jobs" >/dev/null

cp "$work"/rtps-build/eclipse_cyclone-*_shape_main_linux "$out/${product}_shape_main_linux"
cp "$work"/xtypes-build/eclipse_cyclone-*_test_main_linux "$out/${product}_test_main_linux"
for f in "$out/${product}_shape_main_linux" "$out/${product}_test_main_linux"; do
    # Static Cyclone: nothing of it may be left to the dynamic loader.
    if ldd "$f" | grep -q -i 'ddsc\|cyclone'; then
        echo "$f links a Cyclone library dynamically" >&2
        exit 1
    fi
done
echo "cyclonedds $commit $product: $out/${product}_shape_main_linux $out/${product}_test_main_linux"
