#!/usr/bin/env bash
# packaging/linux/deb/build-rmw-deb.sh
# Builds ros-<distro>-rmw-zerodds-cpp: the ROS 2 RMW implementation
# rmw_zerodds_cpp as a Debian package, out of the ament package
# crates/rmw-zerodds-shim/ament.
# Spec: zerodds-ros2-bridge-1.0.md §11 — "One .deb package per ROS distro:
# ros-humble-rmw-zerodds-cpp, …".
#
# The file list is NOT written here. `cmake --install` of the ament package is
# the only producer of it — the rule assemble-debs.sh follows for zerodds-dev
# — and the ament index entries are written by register_rmw_implementation()
# inside that build. What ends up in the .deb is the /opt/ros/<distro> tree the
# RMW packages on packages.ros.org are made of:
#
#   /opt/ros/<distro>/lib/librmw_zerodds_cpp.so       the library RMW_IMPLEMENTATION loads
#   /opt/ros/<distro>/lib/librmw_zerodds.so           Rust DDS bridge
#   /opt/ros/<distro>/lib/libzerodds.so               ZeroDDS C API
#   /opt/ros/<distro>/include/rmw_zerodds_cpp/*.h    REP-2007/REP-2008 headers
#   /opt/ros/<distro>/share/ament_index/resource_index/**/rmw_zerodds_cpp
#   /opt/ros/<distro>/share/rmw_zerodds_cpp/package.xml
#
# The two Rust libraries travel with the package because the RMW links them by
# name next to itself: ament/CMakeLists.txt sets INSTALL_RPATH=$ORIGIN, and
# /opt/ros/<distro>/lib is not on the loader's default search path — ROS puts
# it there through its own setup.sh, as it does for every ros-<distro>-*
# package. Hence also no postinst: no daemon to start, nothing to reload,
# nothing for ldconfig that the ROS setup does not already do.
#
# Usage:
#   packaging/linux/deb/build-rmw-deb.sh --distro jazzy --out dist-deb
#
# Needs a ROS 2 install for <distro> with dev headers (default
# /opt/ros/<distro>), plus cargo, cmake and dpkg-deb. Both callers run inside
# ros:<distro>-ros-base: the weekly `ros-rmw-deb` job in ci/full.yml and the
# builder stage of packaging/docker/ros2-shim/Dockerfile.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$HERE/../../.." && pwd)"

# The supported distros: zerodds-ros2-bridge-1.0.md §10 ("A separate build
# target per distro") plus Kilted, the LTS between Jazzy and Rolling. A name outside
# this set is refused, not guessed: the package name and the install prefix
# both come from it, and a wrong one produces a .deb that installs itself into
# a prefix nothing sources.
DISTROS="humble jazzy kilted rolling"

DISTRO="jazzy"
VERSION=""
OUT=""
ROS_PREFIX=""
INSTALL_PREFIX_ROOT="/opt/ros"

die() { echo "build-rmw-deb.sh: $*" >&2; exit 1; }

while [ $# -gt 0 ]; do
    case "$1" in
        --distro)      DISTRO="$2";     shift 2;;
        --version)     VERSION="$2";    shift 2;;
        --out)         OUT="$2";        shift 2;;
        # Optional: a ROS 2 install for the distro other than
        # /opt/ros/<distro> (a RoboStack micromamba prefix, say). The package
        # still installs into /opt/ros/<distro>: that prefix is part of the
        # package name's meaning, not of the build host's layout.
        --ros-prefix)  ROS_PREFIX="$2"; shift 2;;
        *) echo "unknown arg: $1" >&2; exit 1;;
    esac
done

[ -n "$OUT" ] || die "--out required"

known=no
for d in $DISTROS; do
    if [ "$d" = "$DISTRO" ]; then known=yes; fi
done
[ "$known" = yes ] || die "--distro '$DISTRO' is not a ROS distro this tree builds (known: $DISTROS)"

PKG="ros-${DISTRO}-rmw-zerodds-cpp"
INSTALL_PREFIX="${INSTALL_PREFIX_ROOT}/${DISTRO}"
ROS_PREFIX="${ROS_PREFIX:-${INSTALL_PREFIX}}"
[ -d "$ROS_PREFIX" ] || die "ROS prefix $ROS_PREFIX not found (sourced /opt/ros/$DISTRO/setup.sh in an image, or pass --ros-prefix)"

ARCH="$(uname -m)"
case "$ARCH" in
    x86_64)  DEB_ARCH="amd64";;
    aarch64) DEB_ARCH="arm64";;
    *) die "unsupported arch: $ARCH (no Debian architecture for it here)";;
esac

# The version is the caller's (a git tag, as publish-deb.yml passes one to
# assemble-debs.sh) and falls back to the workspace version. The same string
# goes into the control field and into the package.xml of the tree, so there is
# one version to read; the Debian revision, if the caller wants one, is part of
# it the way it is for every other zerodds-* .deb.
if [ -z "$VERSION" ]; then
    VERSION="$(sed -n '/^\[workspace\.package\]/,/^\[/ s/^version *= *"\(.*\)"/\1/p' \
               "$REPO_ROOT/Cargo.toml")"
    [ -n "$VERSION" ] || die "no --version and no [workspace.package] version in $REPO_ROOT/Cargo.toml"
fi
printf '%s' "$VERSION" | grep -Eq '^(0|[1-9][0-9]*:)?[0-9][A-Za-z0-9.+~:-]*$' \
    || die "--version '$VERSION' is not a Debian version (upstream part starts with a digit, revision after a hyphen)"

mkdir -p "$OUT"
OUT="$(cd "$OUT" && pwd)"

# ROOT is the scratch space; `stage` is the file root of the package and holds
# nothing but the install prefix. The cmake build tree stays in ROOT/build: a
# .deb carrying CMakeCache.txt, object files and an absolute build path is a
# .deb nobody can reproduce. Step 4 checks that separation.
ROOT="$(mktemp -d)"
stage="$ROOT/stage"
mkdir -p "$stage"
trap 'rm -rf "$ROOT"' EXIT

# 1) The two Rust libraries the RMW links: the DDS bridge
# (librmw_zerodds) and the C API (libzerodds).
#
# CARGO_TARGET_DIR is pinned to $REPO_ROOT/target because that is where
# ament/CMakeLists.txt looks for them (`${ZERODDS_REPO_ROOT}/target/release`,
# its ZERODDS_LIB_DIR). A caller's CARGO_TARGET_DIR pointing elsewhere would
# build them here and link none of them there.
echo "== 1) cargo build -p rmw-zerodds-shim -p zerodds-c-api --release"
(
    cd "$REPO_ROOT"
    CARGO_TARGET_DIR="$REPO_ROOT/target" \
        cargo build --release -p rmw-zerodds-shim -p zerodds-c-api
)

# 2) The ament package rmw_zerodds_cpp, configured against that ROS install
# and staged into the package root: DESTDIR puts the /opt/ros/<distro> tree
# under $stage, which is the layout dpkg-deb wants. `--prefix "$stage"` would
# replace the prefix and leave share/ at the top of $stage (the CI job read
# no package.xml under opt/ros/<distro> because of that).
echo "== 2) cmake configure/build/install rmw_zerodds_cpp into $stage"
ZERODDS_HOME="$REPO_ROOT" cmake -S "$REPO_ROOT/crates/rmw-zerodds-shim/ament" \
    -B "$ROOT/build" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_PREFIX_PATH="$ROS_PREFIX" \
    -DCMAKE_INSTALL_PREFIX="$INSTALL_PREFIX"
cmake --build "$ROOT/build"
DESTDIR="$stage" cmake --install "$ROOT/build"

# 3) package.xml says the ament project's own version (0.0.0, crates/
# rmw-zerodds-shim/ament/package.xml). The version this package is released
# under is stamped in so the manifest in the file and the control field agree;
# a file where the pattern is absent is an error, not a skipped edit.
echo "== 3) stamp the release version into the installed package.xml"
PKG_XML="$stage$INSTALL_PREFIX/share/rmw_zerodds_cpp/package.xml"
[ -f "$PKG_XML" ] || die "the ament install has no share/rmw_zerodds_cpp/package.xml (ament_package() did not run)"
sed "s|<version>.*</version>|<version>${VERSION}</version>|" "$PKG_XML" > "$PKG_XML.new"
grep -q "<version>${VERSION}</version>" "$PKG_XML.new" \
    || die "could not stamp $VERSION into $PKG_XML (no <version> element to replace)"
mv "$PKG_XML.new" "$PKG_XML"

# 4) The file list the control field promises, checked on the staging tree
# before it becomes a .deb. Missing → error naming the path, never a .deb that
# installs an RMW rcl cannot find.
echo "== 4) verify the staged file list"
staged_top="$(cd "$stage" && ls -A)"
[ "$staged_top" = "opt" ] \
    || die "the package file root holds '$staged_top', expected only the install prefix (opt)"
LIB="$stage$INSTALL_PREFIX/lib/librmw_zerodds_cpp.so"
[ -f "$LIB" ] || die "$LIB not installed — the ament build did not produce the RMW library"
IDX="$stage$INSTALL_PREFIX/share/ament_index/resource_index"
for res in packages rmw_typesupport; do
    [ -f "$IDX/$res/rmw_zerodds_cpp" ] \
        || die "$IDX/$res/rmw_zerodds_cpp missing — rmw_implementation_cmake did not register the implementation"
done
# The libraries the RMW resolves through RPATH $ORIGIN, in the same
# directory. Without them the .deb installs but every rcl node fails to load.
for so in librmw_zerodds.so libzerodds.so; do
    [ -f "$stage$INSTALL_PREFIX/lib/$so" ] \
        || die "$stage$INSTALL_PREFIX/lib/$so missing — the cargo build in step 1 did not produce it"
done

# 5) Assemble.
#
# Section/Priority as the packages on packages.ros.org carry them.
#
# Depends names the one library that is neither ROS nor libc: libzerodds.so
# links OpenSSL. The ROS libraries the RMW links (librmw, librcutils,
# rmw_dds_common, the rosidl_typesupport_* set) belong to ros-<distro>-* dpkg
# packages, and mapping ELF names to those is dpkg-shlibdeps' job — which this
# package, assembled outside debhelper, does not run. It carries the same
# hand-kept Depends assemble-debs.sh keeps for the zerodds-* packages. The
# package name carries the distro, and the package is unusable without that ROS
# installation, exactly like the ros-<distro>-* packages beside it.
echo "== 5) dpkg-deb --build"
mkdir -p "$stage/DEBIAN"
cat > "$stage/DEBIAN/control" <<CONTROL
Package: $PKG
Version: $VERSION
Architecture: $DEB_ARCH
Maintainer: ZeroDDS Contributors <release@zerodds.org>
Section: misc
Priority: optional
Homepage: https://zerodds.org
Depends: libssl3
Description: ROS 2 RMW implementation rmw_zerodds_cpp (ZeroDDS)
 The RMW implementation rmw_zerodds_cpp for ROS 2 $DISTRO, built from the
 ament package in the ZeroDDS repository (crates/rmw-zerodds-shim/ament):
 librmw_zerodds_cpp.so plus the Rust DDS bridge and the ZeroDDS C API it
 links by name, the ament index entries that register it and its package.xml.
 Selected with RMW_IMPLEMENTATION=rmw_zerodds_cpp.
 .
 Install prefix /opt/ros/$DISTRO, as for every other ros-$DISTRO-* package.
 Spec: zerodds-ros2-bridge-1.0.md §11.
CONTROL

DEB="$OUT/${PKG}_${VERSION}_${DEB_ARCH}.deb"
dpkg-deb --build --root-owner-group "$stage" "$DEB"
echo "built $DEB"