#!/usr/bin/env bash
# packaging/github-actions/aur-publish.sh
# Writes PKGBUILD + .SRCINFO of the AUR packages zerodds-bin and zerodds for
# a release tag and pushes them to the AUR.
#
# Usage:
#   aur-publish.sh <tag>              render, commit and push (~/.ssh/aur)
#   aur-publish.sh <tag> --out <dir>  render into <dir>/zerodds-bin and
#                                     <dir>/zerodds, no git, no push
#
# - zerodds-bin unpacks the .deb packages of the GitHub release.
# - zerodds builds the same .deb packages from the tag's source archive with
#   packaging/linux/deb/assemble-debs.sh and unpacks them, so both packages
#   install the same files.
# Every source carries its sha256, computed here from the release files.
# .SRCINFO is what `makepkg --printsrcinfo` prints, so the script runs on Arch
# Linux as a user other than root (release: archlinux:base-devel container).
#
# AUR pkgver may only contain [a-zA-Z0-9._] — no '-'.
# v1.0.0           → 1.0.0
# v1.0.0-rc.7      → 1.0.0_rc.7
# v1.0.0-beta.5    → 1.0.0_beta.5
# v1.0.0-alpha.1+x → 1.0.0_alpha.1.x  (build-metadata stripped)

set -euo pipefail

TAG="${1:-}"
if [ -z "$TAG" ]; then
    echo "usage: aur-publish.sh <tag> [--out <dir>]" >&2
    exit 1
fi
OUT_DIR=""
if [ "${2:-}" = "--out" ]; then
    OUT_DIR="${3:?--out needs a directory}"
fi

PKGVER=$(echo "$TAG" | sed -e 's/^v//' -e 's/-/_/g' -e 's/+/./g')
TAG_NO_V="${TAG#v}"
RELEASE="https://github.com/zero-objects/zero-dds/releases/download/${TAG}"
ARCHIVE="https://github.com/zero-objects/zero-dds/archive/refs/tags/${TAG}.tar.gz"
# GitHub names the top directory of the archive <repo>-<tag without v>.
SOURCE_DIR="zero-dds-${TAG_NO_V}"
# The sub-packages of packaging/linux/deb/assemble-debs.sh.
DEBS=(core cli dev ws-bridge mqtt-bridge coap-bridge amqp-bridge grpc-bridge
      corba-bridge zenoh-bridge opcua-bridge webdds-bridge ros2 router
      durability naming xrce-agent)

WORK=$(mktemp -d -t aur.XXXXX)
trap 'rm -rf "$WORK"' EXIT

sha256_of () {
    curl -fsSL --retry 3 -o "$WORK/download" "$1"
    sha256sum "$WORK/download" | cut -d' ' -f1
}

# Per architecture: the source URLs and their sha256, in the same order.
declare -A SOURCES SUMS
for arch in x86_64 aarch64; do
    deb_arch=amd64
    [ "$arch" = aarch64 ] && deb_arch=arm64
    for p in "${DEBS[@]}"; do
        url="${RELEASE}/zerodds-${p}_${TAG_NO_V}_${deb_arch}.deb"
        SOURCES[$arch]+="${url}"$'\n'
        SUMS[$arch]+="$(sha256_of "$url")"$'\n'
    done
done
ARCHIVE_SUM=$(sha256_of "$ARCHIVE")

# Prints `name=(` + one quoted item per line + `)`.
bash_array () {
    local name="$1" items="$2" item
    printf '%s=(\n' "$name"
    while IFS= read -r item; do
        [ -n "$item" ] && printf "  '%s'\n" "$item"
    done <<< "$items"
    printf ')\n'
}

# Unpacks the data archive of every .deb in the given directory into $pkgdir.
# bsdtar (libarchive, part of base) reads the ar container of a .deb.
UNPACK_DEBS='    local deb
    for deb in "$1"/*.deb; do
        bsdtar -xOf "$deb" '"'"'data.tar.*'"'"' | bsdtar -xpf - -C "${pkgdir}"
    done'

render_bin () {
    local dir="$1"
    {
        cat <<EOF
# Maintainer: ZeroDDS Contributors <admin@zerodds.org>
pkgname=zerodds-bin
pkgver=${PKGVER}
pkgrel=1
pkgdesc="Pure-Rust OMG Data Distribution Service implementation (precompiled binaries)"
arch=('x86_64' 'aarch64')
url="https://zerodds.org"
license=('Apache-2.0')
provides=('zerodds')
conflicts=('zerodds')
depends=('glibc' 'gcc-libs')
# The binaries stay as the release built them.
options=('!strip' '!debug')
EOF
        bash_array source_x86_64 "${SOURCES[x86_64]}"
        bash_array source_aarch64 "${SOURCES[aarch64]}"
        bash_array sha256sums_x86_64 "${SUMS[x86_64]}"
        bash_array sha256sums_aarch64 "${SUMS[aarch64]}"
        # makepkg would unpack every .deb into the same srcdir, each one
        # overwriting the data.tar.* of the previous one.
        printf 'noextract=(\n'
        printf '%s\n%s\n' "${SOURCES[x86_64]}" "${SOURCES[aarch64]}" |
            sed -n 's|.*/||p' | sed "s|.*|  '&'|"
        printf ')\n\n'
        printf '_unpack_debs() {\n%s\n}\n\n' "$UNPACK_DEBS"
        cat <<'EOF'
package() {
    _unpack_debs "${srcdir}"
}
EOF
    } > "$dir/PKGBUILD"
    (cd "$dir" && makepkg --printsrcinfo > .SRCINFO)
}

render_src () {
    local dir="$1"
    {
        cat <<EOF
# Maintainer: ZeroDDS Contributors <admin@zerodds.org>
pkgname=zerodds
pkgver=${PKGVER}
pkgrel=1
pkgdesc="Pure-Rust OMG Data Distribution Service implementation (built from source)"
arch=('x86_64' 'aarch64')
url="https://zerodds.org"
license=('Apache-2.0')
provides=('zerodds')
conflicts=('zerodds-bin')
depends=('glibc' 'gcc-libs')
# clang: bindgen of iceoryx2-pal-posix loads libclang.
makedepends=('rust' 'clang' 'cmake' 'dpkg' 'pkgconf' 'openssl')
options=('!lto' '!debug')
source=("zerodds-\${pkgver}.tar.gz::${ARCHIVE}")
sha256sums=('${ARCHIVE_SUM}')
_source_dir=${SOURCE_DIR}
_tag_version=${TAG_NO_V}

EOF
        printf '_unpack_debs() {\n%s\n}\n\n' "$UNPACK_DEBS"
        cat <<'EOF'
prepare() {
    cd "${_source_dir}"
    cargo fetch --locked --target "${CARCH}-unknown-linux-gnu"
}

build() {
    cd "${_source_dir}"
    # .cargo/config.toml names the cross linker of the release build for
    # aarch64; here the build runs on the target itself.
    export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=cc
    # The command of .github/workflows/publish-deb.yml.
    cargo build --frozen --release --workspace --target "${CARCH}-unknown-linux-gnu" \
        --features zerodds-corba-cosnaming/daemon,zerodds-xrce-agent/daemon
}

package() {
    cd "${_source_dir}"
    bash packaging/linux/deb/assemble-debs.sh --target "${CARCH}-unknown-linux-gnu" \
        --version "${_tag_version}" --out "${srcdir}/debs"
    _unpack_debs "${srcdir}/debs"
}
EOF
    } > "$dir/PKGBUILD"
    (cd "$dir" && makepkg --printsrcinfo > .SRCINFO)
}

if [ -n "$OUT_DIR" ]; then
    mkdir -p "$OUT_DIR/zerodds-bin" "$OUT_DIR/zerodds"
    render_bin "$OUT_DIR/zerodds-bin"
    render_src "$OUT_DIR/zerodds"
    echo "Rendered zerodds-bin and zerodds for ${TAG} into ${OUT_DIR}"
    exit 0
fi

for pkg in zerodds-bin zerodds; do
    cd "$WORK"
    git clone "ssh://aur@aur.archlinux.org/${pkg}.git"
    cd "$pkg"
    if [ "$pkg" = zerodds-bin ]; then render_bin .; else render_src .; fi
    git add PKGBUILD .SRCINFO
    git -c user.email=admin@zerodds.org -c user.name="zerodds-release-bot" \
        commit -m "Update to ${TAG}" || echo "(no changes for ${pkg})"
    git push origin master
done

echo "AUR push complete for ${TAG}"
