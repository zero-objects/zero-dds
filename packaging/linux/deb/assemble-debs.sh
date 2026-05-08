#!/usr/bin/env bash
# packaging/linux/deb/assemble-debs.sh
# Assembles all zerodds-*.deb sub-packages from a release-build.
# Called by .github/workflows/release.yml -> publish-deb.yml.
# Spec: zerodds-deployment-1.0.md §2.2.1.
set -euo pipefail

TARGET=""
VERSION=""
OUT=""

while [ $# -gt 0 ]; do
    case "$1" in
        --target)  TARGET="$2";  shift 2;;
        --version) VERSION="$2"; shift 2;;
        --out)     OUT="$2";     shift 2;;
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

build_pkg () {
    local name="$1"        # z.B. zerodds-ws-bridge
    local daemon="$2"      # ws-bridged | "" fuer core/cli/dev
    local stage
    stage=$(mktemp -d -p "$ROOT")

    mkdir -p "$stage/DEBIAN"
    sed -e "s/@DAEMON@/$daemon/g" \
        packaging/linux/deb/postinst.tmpl > "$stage/DEBIAN/postinst"
    sed -e "s/@DAEMON@/$daemon/g" \
        packaging/linux/deb/prerm.tmpl    > "$stage/DEBIAN/prerm"
    sed -e "s/@DAEMON@/$daemon/g" \
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
Depends: libssl3
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
            # Bin-Liste auf real-existierende Workspace-bins beschraenkt;
            # alte Eintraege (bench/spy/snitch/secure-*/typeobject/xml-config)
            # sind aus dem Workspace entfernt.
            for tool in admin idlc replay perf xmlc traceability lint chaos \
                        dashboard recorder-bridge ros2-shim; do
                src="target/$TARGET/release/zerodds-$tool"
                if [ -f "$src" ]; then
                    install -Dm0755 "$src" "$stage/usr/bin/zerodds-$tool"
                fi
            done
            ;;
        zerodds-dev)
            install -Dm0644 crates/zerodds-c-api/include/zerodds.h \
                "$stage/usr/include/zerodds.h"
            install -Dm0644 packaging/linux/rpm/zerodds.pc \
                "$stage/usr/lib/pkgconfig/zerodds.pc"
            ;;
        zerodds-*-bridge|zerodds-ros2)
            local bin="zerodds-$daemon"
            install -Dm0755 "target/$TARGET/release/$bin" \
                "$stage/usr/bin/$bin"
            install -Dm0644 "packaging/linux/systemd/zerodds-$daemon.service" \
                "$stage/usr/lib/systemd/system/zerodds-$daemon.service"
            install -Dm0640 "packaging/linux/configs/$daemon.yaml.example" \
                "$stage/usr/share/zerodds/configs/$daemon.yaml.example"
            install -Dm0644 "man/man1/zerodds-$daemon.1" \
                "$stage/usr/share/man/man1/zerodds-$daemon.1"
            install -Dm0644 "man/man5/zerodds-$daemon.yaml.5" \
                "$stage/usr/share/man/man5/zerodds-$daemon.yaml.5"
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

echo "Built $(ls "$OUT" | wc -l) .deb files in $OUT"
