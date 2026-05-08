#!/usr/bin/env bash
# packaging/github-actions/aur-publish.sh
# Push updated PKGBUILD + .SRCINFO to AUR for zerodds-bin and zerodds.
#
# Inputs:
#   $1 — release tag (e.g. v1.0.0-rc.1)
#
# Required env (set by release.yml):
#   ~/.ssh/aur — private SSH key registered with the fishermen21 AUR account
set -euo pipefail

TAG="${1:-}"
if [ -z "$TAG" ]; then
    echo "usage: aur-publish.sh <tag>" >&2
    exit 1
fi

# AUR-pkgver darf nur [a-zA-Z0-9._] enthalten, kein '-'. Also v1.0.0-rc.1 → 1.0.0_rc1.
PKGVER=$(echo "$TAG" | sed -e 's/^v//' -e 's/-rc\./_rc/' -e 's/-/_/g' -e 's/\./_/g; s/_/./g')
# Reconstruct correct: v1.0.0-rc.1 → 1.0.0 and rc.1 → _rc1
PKGVER=$(echo "$TAG" | sed -e 's/^v//' -e 's/-rc\.\([0-9]*\)$/_rc\1/')
TAG_NO_V="${TAG#v}"

WORK=$(mktemp -d -t aur.XXXXX)
trap 'rm -rf "$WORK"' EXIT

# ============================================================
# zerodds-bin (precompiled binaries from GH Release)
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
source_x86_64=("https://github.com/zero-objects/zero-dds/releases/download/${TAG}/zerodds-${TAG_NO_V}-x86_64-unknown-linux-gnu.tar.gz")
source_aarch64=("https://github.com/zero-objects/zero-dds/releases/download/${TAG}/zerodds-${TAG_NO_V}-aarch64-unknown-linux-gnu.tar.gz")
sha256sums_x86_64=('SKIP')
sha256sums_aarch64=('SKIP')

package() {
    cd "\${srcdir}"
    install -dm755 "\${pkgdir}/usr/bin"
    local bin
    for bin in zerodds-{ws,mqtt,coap,amqp,grpc,corba}-bridged \\
               zerodds-{admin,idlc,xmlc,record,replay,bench,monitor,mq,pcap,perf} \\
               zerodds-ros2-shim; do
        if [[ -f "\$bin" ]]; then
            install -m755 "\$bin" "\${pkgdir}/usr/bin/\${bin}"
        fi
    done
    if [[ -f libzerodds.so ]]; then
        install -dm755 "\${pkgdir}/usr/lib"
        install -m755 libzerodds.so "\${pkgdir}/usr/lib/libzerodds.so"
    fi
    if [[ -f zerodds.h ]]; then
        install -dm755 "\${pkgdir}/usr/include"
        install -m644 zerodds.h "\${pkgdir}/usr/include/"
    fi
    install -Dm644 LICENSE "\${pkgdir}/usr/share/licenses/\${_pkgname}/LICENSE" 2>/dev/null || true
}
EOF

# .SRCINFO — minimal valid form (mksrcinfo would be ideal but we don't pull
# pacman into the GH runner; this hand-rolled version is byte-stable).
cat > .SRCINFO <<EOF
pkgbase = zerodds-bin
	pkgdesc = Pure-Rust OMG Data Distribution Service implementation (precompiled binaries)
	pkgver = ${PKGVER}
	pkgrel = 1
	url = https://zerodds.org
	arch = x86_64
	arch = aarch64
	license = Apache-2.0
	provides = zerodds
	conflicts = zerodds
	depends = glibc
	depends = gcc-libs
	source_x86_64 = https://github.com/zero-objects/zero-dds/releases/download/${TAG}/zerodds-${TAG_NO_V}-x86_64-unknown-linux-gnu.tar.gz
	sha256sums_x86_64 = SKIP
	source_aarch64 = https://github.com/zero-objects/zero-dds/releases/download/${TAG}/zerodds-${TAG_NO_V}-aarch64-unknown-linux-gnu.tar.gz
	sha256sums_aarch64 = SKIP

pkgname = zerodds-bin
EOF

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
    cd "zero-dds-${TAG_NO_V}"
    cargo fetch --locked
}

build() {
    cd "zero-dds-${TAG_NO_V}"
    cargo build --frozen --release --workspace
}

package() {
    cd "zero-dds-${TAG_NO_V}"
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

cat > .SRCINFO <<EOF
pkgbase = zerodds
	pkgdesc = Pure-Rust OMG Data Distribution Service implementation (built from source)
	pkgver = ${PKGVER}
	pkgrel = 1
	url = https://zerodds.org
	arch = x86_64
	arch = aarch64
	license = Apache-2.0
	makedepends = rust>=1.88
	makedepends = cargo
	makedepends = git
	makedepends = pkg-config
	makedepends = openssl
	depends = glibc
	depends = gcc-libs
	options = !lto
	source = zerodds-${PKGVER}.tar.gz::https://github.com/zero-objects/zero-dds/archive/refs/tags/${TAG}.tar.gz
	sha256sums = SKIP

pkgname = zerodds
EOF

git add PKGBUILD .SRCINFO
git -c user.email=mail@sandra-kessler.net -c user.name="zerodds-release-bot" \
    commit -m "Update to ${TAG}" || echo "(no changes for zerodds)"
git push origin master

echo "AUR push complete for ${TAG}"
