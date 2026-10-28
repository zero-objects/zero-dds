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
    local name="$1"        # e.g. zerodds-ws-bridge
    local daemon="$2"      # ws-bridged | "" for core/cli/dev
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
            # The runtime configuration every program reads: its page and a
            # template with every key at its default.
            install -Dm0644 packaging/linux/configs/zerodds.yaml.example \
                "$stage/usr/share/zerodds/configs/zerodds.yaml.example"
            install -Dm0644 man/man5/zerodds.yaml.5 \
                "$stage/usr/share/man/man5/zerodds.yaml.5"
            ;;
        zerodds-cli)
            # Bin list restricted to workspace bins that actually exist.
            # 7 user-facing CLIs (record/bench/monitor/spy/snitch/pcap/mq)
            # plus the internal build tools (admin/idlc/xmlc/etc.).
            for tool in admin idlc replay perf xmlc traceability lint chaos \
                        dashboard recorder-bridge secure-permissions \
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
        zerodds-ros2)
            # The diagnostics CLI of the RMW, its pages and the ros2 drop-in
            # template. No service: rcl loads the RMW into each node.
            install -Dm0755 "target/$TARGET/release/zerodds-ros2-shim" \
                "$stage/usr/bin/zerodds-ros2-shim"
            install -Dm0644 packaging/linux/configs/ros2.yaml.example \
                "$stage/usr/share/zerodds/configs/ros2.yaml.example"
            install -Dm0644 man/man1/zerodds-ros2-shim.1 \
                "$stage/usr/share/man/man1/zerodds-ros2-shim.1"
            install -Dm0644 man/man5/zerodds-ros2.yaml.5 \
                "$stage/usr/share/man/man5/zerodds-ros2.yaml.5"
            ;;
        zerodds-*-bridge)
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
        zerodds-durability)
            # Durability service (TRANSIENT/PERSISTENT), default build: the
            # sqlite and file stores, no DuckDB (feature `lakehouse` is opt-in).
            install -Dm0755 "target/$TARGET/release/zerodds-durability-svc" \
                "$stage/usr/bin/zerodds-durability-svc"
            install -Dm0644 packaging/linux/systemd/zerodds-durability-svc.service \
                "$stage/usr/lib/systemd/system/zerodds-durability-svc.service"
            install -Dm0644 man/man1/zerodds-durability-svc.1 \
                "$stage/usr/share/man/man1/zerodds-durability-svc.1"
            ;;
        zerodds-naming)
            # Standalone OMG CosNaming 1.3 server.
            install -Dm0755 "target/$TARGET/release/zerodds-naming-svc" \
                "$stage/usr/bin/zerodds-naming-svc"
            install -Dm0644 packaging/linux/systemd/zerodds-naming-svc.service \
                "$stage/usr/lib/systemd/system/zerodds-naming-svc.service"
            install -Dm0640 packaging/linux/configs/naming-svc.yaml.example \
                "$stage/usr/share/zerodds/configs/naming-svc.yaml.example"
            install -Dm0644 man/man1/zerodds-naming-svc.1 \
                "$stage/usr/share/man/man1/zerodds-naming-svc.1"
            install -Dm0644 man/man5/zerodds-naming-svc.yaml.5 \
                "$stage/usr/share/man/man5/zerodds-naming-svc.yaml.5"
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
        zerodds-xrce-agent)
            # DDS-XRCE agent daemon: bridges eProsima-compatible DDS-XRCE
            # Micro clients (UDP/TCP/serial) into the DDS data space. Own
            # case because the binary/config-stem (zerodds-xrce-agentd /
            # xrce-agentd.yaml) don't match the zerodds-*-bridge glob's
            # $daemon-based naming, same reason router/durability have
            # their own case above.
            install -Dm0755 "target/$TARGET/release/zerodds-xrce-agentd" \
                "$stage/usr/bin/zerodds-xrce-agentd"
            install -Dm0644 packaging/linux/systemd/zerodds-xrce-agentd.service \
                "$stage/usr/lib/systemd/system/zerodds-xrce-agentd.service"
            install -Dm0640 packaging/linux/configs/xrce-agentd.yaml.example \
                "$stage/usr/share/zerodds/configs/xrce-agentd.yaml.example"
            install -Dm0644 man/man1/zerodds-xrce-agentd.1 \
                "$stage/usr/share/man/man1/zerodds-xrce-agentd.1"
            install -Dm0644 man/man5/zerodds-xrce-agentd.yaml.5 \
                "$stage/usr/share/man/man5/zerodds-xrce-agentd.yaml.5"
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
build_pkg zerodds-zenoh-bridge zenoh-bridged
build_pkg zerodds-opcua-bridge opcua-bridged
build_pkg zerodds-webdds-bridge webdds-bridged
build_pkg zerodds-ros2         ""
build_pkg zerodds-router       router         router.json
build_pkg zerodds-durability   durability-svc
build_pkg zerodds-naming       naming-svc
build_pkg zerodds-xrce-agent   xrce-agentd

echo "Built $(ls "$OUT" | wc -l) .deb files in $OUT"
