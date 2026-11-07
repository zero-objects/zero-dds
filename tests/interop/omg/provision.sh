#!/usr/bin/env bash
# The OMG suites and the vendor executables they run, in one directory:
#
#   provision.sh <dest> [--cyclone-master <work dir>]
#
#   <dest>/dds-rtps, dds-xtypes, dds-security   the suites at the commits of pins.tsv
#   <dest>/vendors/<suite>/<executable>         the vendor executables of vendors.tsv,
#                                               each checked against its sha256
#   <dest>/vendors.tsv                          the list they came from
#   <dest>/versions.txt                         what is in <dest>
#
# With --cyclone-master, Cyclone DDS master's two suite clients, built from source
# at the commit of pins.tsv by build-cyclone-suite-clients.sh in <work dir>, go
# beside them (a C compiler, cmake and bison needed).
#
# ci/Dockerfile.omg (the ci-omg image of GitLab, /opt/omg) and the interop
# workflow of GitHub run this script; the image builds the master clients in a
# stage of its own and adds them after. The caller appends the listing of
# <dest>/vendors to versions.txt.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
dest=${1:?usage: provision.sh <dest> [--cyclone-master <work dir>]}
shift
master_work=""
if [ "${1:-}" = --cyclone-master ]; then
  master_work=${2:?usage: provision.sh <dest> --cyclone-master <work dir>}
fi
mkdir -p "$dest"
dest=$(cd "$dest" && pwd)

pin() { awk -F'\t' -v k="$1" -v n="$2" '!/^#/ && $1 == k && $2 == n { print $3 }' "$HERE/pins.tsv"; }

: > "$dest/versions.txt"
for suite in dds-rtps dds-xtypes dds-security; do
  commit=$(pin suite "$suite")
  [ -n "$commit" ] || { echo "pins.tsv: no commit for $suite" >&2; exit 1; }
  rm -rf "${dest:?}/$suite" "$dest/$suite-$commit"
  curl -sSfL "https://codeload.github.com/omg-dds/$suite/tar.gz/$commit" | tar xz -C "$dest"
  mv "$dest/$suite-$commit" "$dest/$suite"
  echo "$suite $commit" >> "$dest/versions.txt"
done

cp "$HERE/vendors.tsv" "$dest/vendors.tsv"
grep -v '^#' "$HERE/vendors.tsv" | while IFS=$'\t' read -r suite release asset sha; do
  tmp=$(mktemp)
  curl -sSfL -o "$tmp" "https://github.com/omg-dds/$suite/releases/download/$release/$asset"
  if ! echo "$sha  $tmp" | sha256sum -c - > /dev/null; then
    echo "$asset: the sha256 is not the one vendors.tsv names" >&2
    rm -f "$tmp"
    exit 1
  fi
  mkdir -p "$dest/vendors/$suite"
  unzip -q -o "$tmp" -d "$dest/vendors/$suite"
  rm -f "$tmp"
  echo "$suite $release $asset sha256:$sha" >> "$dest/versions.txt"
done
find "$dest/vendors" -type f \( -name '*_main_linux' -o -name '*_linux' \) -exec chmod +x {} +

if [ -n "$master_work" ]; then
  commit=$(pin source cyclonedds-master)
  [ -n "$commit" ] || { echo "pins.tsv: no commit for cyclonedds-master" >&2; exit 1; }
  bash "$HERE/build-cyclone-suite-clients.sh" "$commit" "$dest/dds-rtps" "$dest/dds-xtypes" \
    "$master_work" "$master_work/out" eclipse_cyclone-master
  cp "$master_work/out/eclipse_cyclone-master_shape_main_linux" "$dest/vendors/dds-rtps/"
  cp "$master_work/out/eclipse_cyclone-master_test_main_linux" "$dest/vendors/dds-xtypes/"
  echo "cyclonedds master $commit, built: eclipse_cyclone-master_shape_main_linux eclipse_cyclone-master_test_main_linux" \
    >> "$dest/versions.txt"
fi
