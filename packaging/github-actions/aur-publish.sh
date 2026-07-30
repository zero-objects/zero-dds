#!/usr/bin/env bash
# packaging/github-actions/aur-publish.sh
# Push updated PKGBUILD + .SRCINFO to AUR for zerodds-bin and zerodds.
#
# Inputs:
#   $1 — release tag (e.g. v1.0.0-rc.7)
#
# Required env (set by release.yml):
#   ~/.ssh/aur — private SSH key registered with the fishermen21 AUR account
#
# Strategy:
# - zerodds-bin: extracts zerodds-cli .deb (cli bundle) + bridge .debs
#   from GH Release. Cargo-dist baut keine monolithische tarball; die
#   .debs sind unsere zentrale binary-distribution.
# - zerodds: builds from source-tarball (github auto-archive).
#
# AUR-pkgver darf nur [a-zA-Z0-9._] enthalten — kein '-'.
# v1.0.0           → 1.0.0
# v1.0.0-rc.7      → 1.0.0_rc.7
# v1.0.0-beta.5    → 1.0.0_beta.5
# v1.0.0-alpha.1+x → 1.0.0_alpha.1.x  (build-metadata stripped)

set -euo pipefail

TAG="${1:-}"
if [ -z "$TAG" ]; then
    echo "usage: aur-publish.sh <tag>" >&2
    exit 1
fi

# Robust pkgver-konvertierung: strip leading v, replace alle '-' mit '_',
# und alle '+' mit '.'. Damit ist jede semver-tag-form AUR-konform.
PKGVER=$(echo "$TAG" | sed -e 's/^v//' -e 's/-/_/g' -e 's/+/./g')
TAG_NO_V="${TAG#v}"

# Cargo-dist Layout: jeder bin als .tar.xz pro target plus .deb/.rpm
# pro logischem package (zerodds-cli, zerodds-ws-bridge, ...).
# Wir bauen AUR-bin aus den .debs weil das die zentrale binary-bundle-form ist.
DEB_BASE="https://github.com/zero-objects/zero-dds/releases/download/${TAG}"

WORK=$(mktemp -d -t aur.XXXXX)
trap 'rm -rf "$WORK"' EXIT

# ============================================================
# zerodds-bin (extract + assemble from .deb packages)
# ============================================================
cd "$WORK"
git clone ssh://aur@aur.archlinux.org/zerodds-bin.git
cd zerodds-bin

cat > PKGBUILD <<EOF
# Maintainer: Sandra Keßler <mail@sandra-kessler.net>
pkgname=zerodds-bin
_pkgname=zerodds
pkgver=${PKGVER}
pkgrel=1
pkgdesc="Pure-Rust OMG Data Distribution Service implementation (precompiled binaries)"
arch=('x86_64' 'aarch64')
url="https://zerodds.org"
license=('Apache-2.0')
provides=('zerodds')
conflicts=('zerodds')
depends=('glibc' 'gcc-libs')
makedepends=('binutils')

# .deb-bundles aus dem GH-release. Jedes .deb enthaelt die fertig
# kompilierten binaries für seine Komponente. Wir extrahieren die
# data.tar.* aus jedem .deb und bauen daraus den pacman-package-tree.
source_x86_64=(
  "${DEB_BASE}/zerodds-cli_${TAG_NO_V}_amd64.deb"
  "${DEB_BASE}/zerodds-ws-bridge_${TAG_NO_V}_amd64.deb"
  "${DEB_BASE}/zerodds-mqtt-bridge_${TAG_NO_V}_amd64.deb"
  "${DEB_BASE}/zerodds-coap-bridge_${TAG_NO_V}_amd64.deb"
  "${DEB_BASE}/zerodds-amqp-bridge_${TAG_NO_V}_amd64.deb"
  "${DEB_BASE}/zerodds-grpc-bridge_${TAG_NO_V}_amd64.deb"
  "${DEB_BASE}/zerodds-corba-bridge_${TAG_NO_V}_amd64.deb"
  "${DEB_BASE}/zerodds-ros2_${TAG_NO_V}_amd64.deb"
  "${DEB_BASE}/zerodds-core_${TAG_NO_V}_amd64.deb"
  "${DEB_BASE}/zerodds-dev_${TAG_NO_V}_amd64.deb"
)
source_aarch64=(
  "${DEB_BASE}/zerodds-cli_${TAG_NO_V}_arm64.deb"
  "${DEB_BASE}/zerodds-ws-bridge_${TAG_NO_V}_arm64.deb"
  "${DEB_BASE}/zerodds-mqtt-bridge_${TAG_NO_V}_arm64.deb"
  "${DEB_BASE}/zerodds-coap-bridge_${TAG_NO_V}_arm64.deb"
  "${DEB_BASE}/zerodds-amqp-bridge_${TAG_NO_V}_arm64.deb"
  "${DEB_BASE}/zerodds-grpc-bridge_${TAG_NO_V}_arm64.deb"
  "${DEB_BASE}/zerodds-corba-bridge_${TAG_NO_V}_arm64.deb"
  "${DEB_BASE}/zerodds-ros2_${TAG_NO_V}_arm64.deb"
  "${DEB_BASE}/zerodds-core_${TAG_NO_V}_arm64.deb"
  "${DEB_BASE}/zerodds-dev_${TAG_NO_V}_arm64.deb"
)
sha256sums_x86_64=('SKIP' 'SKIP' 'SKIP' 'SKIP' 'SKIP' 'SKIP' 'SKIP' 'SKIP' 'SKIP' 'SKIP')
sha256sums_aarch64=('SKIP' 'SKIP' 'SKIP' 'SKIP' 'SKIP' 'SKIP' 'SKIP' 'SKIP' 'SKIP' 'SKIP')

package() {
    cd "\${srcdir}"
    # Jedes .deb extrahieren: control.tar.* + data.tar.*
    for deb in *.deb; do
        ar x "\$deb" data.tar.xz 2>/dev/null || ar x "\$deb" data.tar.zst 2>/dev/null || ar x "\$deb" data.tar.gz
        if [ -f data.tar.xz ];  then tar -xJf data.tar.xz -C "\${pkgdir}"; rm data.tar.xz;  fi
        if [ -f data.tar.zst ]; then tar --zstd -xf data.tar.zst -C "\${pkgdir}"; rm data.tar.zst; fi
        if [ -f data.tar.gz ];  then tar -xzf data.tar.gz -C "\${pkgdir}"; rm data.tar.gz;  fi
    done
    # LICENSE einmal zentral installieren
    install -dm755 "\${pkgdir}/usr/share/licenses/\${_pkgname}"
    if [ -f "\${pkgdir}/usr/share/doc/zerodds-cli/LICENSE" ]; then
        cp "\${pkgdir}/usr/share/doc/zerodds-cli/LICENSE" "\${pkgdir}/usr/share/licenses/\${_pkgname}/LICENSE"
    fi
}
EOF

# .SRCINFO — von Hand assembliert (mksrcinfo verlangt makepkg in PATH).
{
    printf 'pkgbase = zerodds-bin\n'
    printf '\tpkgdesc = Pure-Rust OMG Data Distribution Service implementation (precompiled binaries)\n'
    printf '\tpkgver = %s\n' "$PKGVER"
    printf '\tpkgrel = 1\n'
    printf '\turl = https://zerodds.org\n'
    printf '\tarch = x86_64\n'
    printf '\tarch = aarch64\n'
    printf '\tlicense = Apache-2.0\n'
    printf '\tmakedepends = binutils\n'
    printf '\tdepends = glibc\n'
    printf '\tdepends = gcc-libs\n'
    printf '\tprovides = zerodds\n'
    printf '\tconflicts = zerodds\n'
    for arch in x86_64 aarch64; do
        deb_arch=amd64
        [ "$arch" = "aarch64" ] && deb_arch=arm64
        for pkg in cli ws-bridge mqtt-bridge coap-bridge amqp-bridge grpc-bridge corba-bridge ros2 core dev; do
            printf '\tsource_%s = %s/zerodds-%s_%s_%s.deb\n' "$arch" "$DEB_BASE" "$pkg" "$TAG_NO_V" "$deb_arch"
            printf '\tsha256sums_%s = SKIP\n' "$arch"
        done
    done
    printf '\n'
    printf 'pkgname = zerodds-bin\n'
} > .SRCINFO

git add PKGBUILD .SRCINFO
git -c user.email=mail@sandra-kessler.net -c user.name="zerodds-release-bot" \
    commit -m "Update to ${TAG}" || echo "(no changes for zerodds-bin)"
git push origin master

# ============================================================
# zerodds (source build)
# ============================================================
cd "$WORK"
git clone ssh://aur@aur.archlinux.org/zerodds.git
cd zerodds

# github auto-archive named: zero-dds-<tag-without-v>.tar.gz
# (repo name is "zero-dds" not "zerodds")
SOURCE_DIR="zero-dds-${TAG_NO_V}"

cat > PKGBUILD <<EOF
# Maintainer: Sandra Keßler <mail@sandra-kessler.net>
pkgname=zerodds
pkgver=${PKGVER}
pkgrel=1
pkgdesc="Pure-Rust OMG Data Distribution Service implementation (built from source)"
arch=('x86_64' 'aarch64')
url="https://zerodds.org"
license=('Apache-2.0')
depends=('glibc' 'gcc-libs')
makedepends=('rust>=1.88' 'cargo' 'git' 'pkg-config' 'openssl')
options=('!lto')
source=("\$pkgname-\$pkgver.tar.gz::https://github.com/zero-objects/zero-dds/archive/refs/tags/${TAG}.tar.gz")
sha256sums=('SKIP')

prepare() {
    cd "${SOURCE_DIR}"
    cargo fetch --locked
}

build() {
    cd "${SOURCE_DIR}"
    cargo build --frozen --release --workspace --exclude zerodds-durability-store-lakehouse --exclude zerodds-durability-service-bin --exclude zerodds-endpoint-codegen --exclude zerodds-endpoint-golden --exclude zerodds-xrce-agent-demo
}

package() {
    cd "${SOURCE_DIR}"
    install -dm755 "\${pkgdir}/usr/bin" "\${pkgdir}/usr/lib" "\${pkgdir}/usr/include"
    local bin
    for bin in zerodds-{ws,mqtt,coap,amqp,grpc,corba}-bridged \\
               zerodds-{admin,idlc,xmlc,record,replay,bench,monitor,mq,pcap,perf} \\
               zerodds-ros2-shim; do
        if [[ -f "target/release/\${bin}" ]]; then
            install -m755 "target/release/\${bin}" "\${pkgdir}/usr/bin/\${bin}"
        fi
    done
    if [[ -f "target/release/libzerodds.so" ]]; then
        install -m755 "target/release/libzerodds.so" "\${pkgdir}/usr/lib/libzerodds.so"
    fi
    if [[ -f "crates/zerodds-c-api/include/zerodds.h" ]]; then
        install -m644 "crates/zerodds-c-api/include/zerodds.h" "\${pkgdir}/usr/include/zerodds.h"
    fi
    install -Dm644 LICENSE "\${pkgdir}/usr/share/licenses/\${pkgname}/LICENSE"
}
EOF

{
    printf 'pkgbase = zerodds\n'
    printf '\tpkgdesc = Pure-Rust OMG Data Distribution Service implementation (built from source)\n'
    printf '\tpkgver = %s\n' "$PKGVER"
    printf '\tpkgrel = 1\n'
    printf '\turl = https://zerodds.org\n'
    printf '\tarch = x86_64\n'
    printf '\tarch = aarch64\n'
    printf '\tlicense = Apache-2.0\n'
    printf '\tmakedepends = rust>=1.88\n'
    printf '\tmakedepends = cargo\n'
    printf '\tmakedepends = git\n'
    printf '\tmakedepends = pkg-config\n'
    printf '\tmakedepends = openssl\n'
    printf '\tdepends = glibc\n'
    printf '\tdepends = gcc-libs\n'
    printf '\toptions = !lto\n'
    printf '\tsource = zerodds-%s.tar.gz::https://github.com/zero-objects/zero-dds/archive/refs/tags/%s.tar.gz\n' "$PKGVER" "$TAG"
    printf '\tsha256sums = SKIP\n'
    printf '\n'
    printf 'pkgname = zerodds\n'
} > .SRCINFO

git add PKGBUILD .SRCINFO
git -c user.email=mail@sandra-kessler.net -c user.name="zerodds-release-bot" \
    commit -m "Update to ${TAG}" || echo "(no changes for zerodds)"
git push origin master

echo "AUR push complete for ${TAG}"
