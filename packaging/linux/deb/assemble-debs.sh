#!/usr/bin/env bash
# packaging/linux/deb/assemble-debs.sh
# Assembles all zerodds-*.deb sub-packages from a release-build.
# Called by .github/workflows/release.yml -> publish-deb.yml.
# Spec: zerodds-deployment-1.0.md §2.2.1.
set -euo pipefail

TARGET=""
VERSION=""
OUT=""
SDK_STAGE=""

while [ $# -gt 0 ]; do
    case "$1" in
        --target)    TARGET="$2";    shift 2;;
        --version)   VERSION="$2";   shift 2;;
        --out)       OUT="$2";       shift 2;;
        # Optional: path to a pre-populated `cmake --install` staging tree
        # (…/usr). When omitted, this script builds it itself (see
        # ensure_sdk_stage). The zerodds-dev sub-package is filled EXCLUSIVELY
        # from that tree — no hand-maintained header copy list (spec P0-4).
        --sdk-stage) SDK_STAGE="$2"; shift 2;;
        *) echo "unknown arg: $1" >&2; exit 1;;
    esac
done

[ -n "$TARGET" ]  || { echo "--target required" >&2; exit 1; }
[ -n "$VERSION" ] || { echo "--version required" >&2; exit 1; }
[ -n "$OUT" ]     || { echo "--out required" >&2; exit 1; }

ARCH="${TARGET%%-*}"
case "$ARCH" in
    x86_64)  DEB_ARCH="amd64";;
    aarch64) DEB_ARCH="arm64";;
    *) echo "unsupported arch: $ARCH" >&2; exit 1;;
esac

mkdir -p "$OUT"
ROOT=$(mktemp -d)
trap 'rm -rf "$ROOT"' EXIT

# Materialise the single SDK install staging tree once. Everything a C/C++
# consumer needs is produced by `cmake --install` (root CMakeLists.txt install
# block) — headers (C + C++), the static library, zeroddsConfig.cmake +
# ConfigVersion, the IDLC generator module, zerodds.pc, and the zerodds-idlc
# host tool. The zerodds-dev package is then a thin copy of this tree, so no
# header can be present in one packaging path but missing from another.
# Static lib on purpose: the runtime shared .so ships in zerodds-core; -dev
# provides the self-contained static SDK + the CMake/pkg-config glue.
ensure_sdk_stage () {
    [ -n "$SDK_STAGE" ] && return 0
    local build
    build="$ROOT/zdw-build"
    SDK_STAGE="$ROOT/zdw-stage/usr"
    cmake -S . -B "$build" \
        -DCMAKE_BUILD_TYPE=Release \
        -DBUILD_SHARED_LIBS=OFF \
        -DZERODDS_BUILD_CPP=ON \
        -DZERODDS_BUILD_IDLC=ON \
        -DZERODDS_BUILD_EXAMPLES=OFF \
        -DZERODDS_BUILD_TESTS=OFF
    cmake --build "$build"
    cmake --install "$build" --prefix "$SDK_STAGE"
}

build_pkg () {
    local name="$1"        # z.B. zerodds-ws-bridge
    local daemon="$2"      # ws-bridged | "" fuer core/cli/dev
    local conf="${3:-$daemon.yaml}"   # config basename; bridges = <daemon>.yaml,
                                      # router = router.json (JSON config)
    local stage
    stage=$(mktemp -d -p "$ROOT")

    # Per-package runtime dependencies. -dev pulls in -cli because the SDK's
    # zeroddsConfig.cmake references /usr/bin/zerodds-idlc, which -cli owns
    # (the host tool is not duplicated into -dev to avoid a dpkg file clash).
    local depends="libssl3"
    case "$name" in
        zerodds-dev) depends="libssl3, zerodds-core (= $VERSION), zerodds-cli (= $VERSION)";;
    esac

    mkdir -p "$stage/DEBIAN"
    sed -e "s/@DAEMON@/$daemon/g" -e "s/@CONF@/$conf/g" \
        packaging/linux/deb/postinst.tmpl > "$stage/DEBIAN/postinst"
    sed -e "s/@DAEMON@/$daemon/g" -e "s/@CONF@/$conf/g" \
        packaging/linux/deb/prerm.tmpl    > "$stage/DEBIAN/prerm"
    sed -e "s/@DAEMON@/$daemon/g" -e "s/@CONF@/$conf/g" \
        packaging/linux/deb/postrm.tmpl   > "$stage/DEBIAN/postrm"
    chmod 0755 "$stage/DEBIAN/postinst" "$stage/DEBIAN/prerm" "$stage/DEBIAN/postrm"

    cat > "$stage/DEBIAN/control" <<CONTROL
Package: $name
Version: $VERSION
Architecture: $DEB_ARCH
Maintainer: ZeroDDS Contributors <release@zerodds.org>
Section: net
Priority: optional
Homepage: https://zerodds.org
Depends: $depends
Description: ZeroDDS sub-package $name
 See zerodds-deployment-1.0.md §2.2.1.
CONTROL

    case "$name" in
        zerodds-core)
            install -Dm0755 target/$TARGET/release/libzerodds.so \
                "$stage/usr/lib/libzerodds.so.1.0.0"
            ln -sf libzerodds.so.1.0.0 "$stage/usr/lib/libzerodds.so.1"
            ln -sf libzerodds.so.1     "$stage/usr/lib/libzerodds.so"
            install -Dm0644 packaging/linux/systemd/zerodds-tmpfiles.conf \
                "$stage/usr/lib/tmpfiles.d/zerodds.conf"
            install -Dm0644 packaging/linux/systemd/zerodds-sysusers.conf \
                "$stage/usr/lib/sysusers.d/zerodds.conf"
            ;;
        zerodds-cli)
            # Bin-Liste auf real-existierende Workspace-bins beschraenkt.
            # 7 user-facing CLIs (record/bench/monitor/spy/snitch/pcap/mq)
            # plus die internen Build-Tools (admin/idlc/xmlc/etc.).
            for tool in admin idlc replay perf xmlc traceability lint chaos \
                        dashboard recorder-bridge ros2-shim \
                        record bench monitor spy snitch pcap mq; do
                src="target/$TARGET/release/zerodds-$tool"
                if [ -f "$src" ]; then
                    install -Dm0755 "$src" "$stage/usr/bin/zerodds-$tool"
                fi
                man="man/man1/zerodds-$tool.1"
                if [ -f "$man" ]; then
                    install -Dm0644 "$man" \
                        "$stage/usr/share/man/man1/zerodds-$tool.1"
                fi
            done
            ;;
        zerodds-dev)
            # Thin adapter over the single SDK staging tree — NO per-file copy
            # list. Everything (C + C++ headers, static lib, zeroddsConfig.cmake
            # + ConfigVersion, IDLC generator module, zerodds.pc) comes from
            # `cmake --install` (spec P0-4).
            ensure_sdk_stage
            mkdir -p "$stage/usr"
            cp -a "$SDK_STAGE/." "$stage/usr/"
            # The zerodds-idlc host tool is owned by zerodds-cli; drop the copy
            # the SDK install placed in bin/ so the two packages do not clash.
            rm -f "$stage/usr/bin/zerodds-idlc"
            rmdir "$stage/usr/bin" 2>/dev/null || true
            ;;
        zerodds-*-bridge|zerodds-ros2)
            local bin="zerodds-$daemon"
            install -Dm0755 "target/$TARGET/release/$bin" \
                "$stage/usr/bin/$bin"
            install -Dm0644 "packaging/linux/systemd/zerodds-$daemon.service" \
                "$stage/usr/lib/systemd/system/zerodds-$daemon.service"
            install -Dm0640 "packaging/linux/configs/$daemon.yaml.example" \
                "$stage/usr/share/zerodds/configs/$daemon.yaml.example"
            if [ -f "man/man1/zerodds-$daemon.1" ]; then
                install -Dm0644 "man/man1/zerodds-$daemon.1" \
                    "$stage/usr/share/man/man1/zerodds-$daemon.1"
            fi
            if [ -f "man/man5/zerodds-$daemon.yaml.5" ]; then
                install -Dm0644 "man/man5/zerodds-$daemon.yaml.5" \
                    "$stage/usr/share/man/man5/zerodds-$daemon.yaml.5"
            fi
            ;;
        zerodds-router)
            # Routing service: own case because its config is JSON
            # (router.json.example), not the bridge <daemon>.yaml.
            install -Dm0755 "target/$TARGET/release/zerodds-router" \
                "$stage/usr/bin/zerodds-router"
            install -Dm0644 packaging/linux/systemd/zerodds-router.service \
                "$stage/usr/lib/systemd/system/zerodds-router.service"
            install -Dm0640 packaging/linux/configs/router.json.example \
                "$stage/usr/share/zerodds/configs/router.json.example"
            if [ -f "man/man1/zerodds-router.1" ]; then
                install -Dm0644 "man/man1/zerodds-router.1" \
                    "$stage/usr/share/man/man1/zerodds-router.1"
            fi
            ;;
    esac

    dpkg-deb --build --root-owner-group "$stage" \
             "$OUT/${name}_${VERSION}_${DEB_ARCH}.deb"
}

build_pkg zerodds-core         ""
build_pkg zerodds-cli          ""
build_pkg zerodds-dev          ""
build_pkg zerodds-ws-bridge    ws-bridged
build_pkg zerodds-mqtt-bridge  mqtt-bridged
build_pkg zerodds-coap-bridge  coap-bridged
build_pkg zerodds-amqp-bridge  amqp-bridged
build_pkg zerodds-grpc-bridge  grpc-bridged
build_pkg zerodds-corba-bridge corba-bridged
build_pkg zerodds-ros2         ros2-shim
build_pkg zerodds-router       router         router.json

echo "Built $(ls "$OUT" | wc -l) .deb files in $OUT"
