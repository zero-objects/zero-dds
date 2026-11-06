#!/usr/bin/env bash
# The four `zerodds_runtime_for_each_*` of the C API, called from a C program
# through `crates/zerodds-c-api/include/zerodds.h` alone:
# crates/zerodds-c-api/tests/for_each_c.c enumerates a writer and a reader of
# two runtimes by topic and by endpoint (GUID, reliability).
#
# The oracle is the C compiler (`-std=c99 -Wall -Wextra -Werror` over the
# header: the callback typedefs and `zerodds_ZeroDdsEndpointInfo` have to be
# declared there) and the program's comparison with the GUIDs
# zerodds_writer_guid / zerodds_reader_guid name.
#
#     bash tests/integration/for_each_c.sh
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# The target directory of tests/integration/sdk_cmake.sh where the runner
# offers a cache: CMake has built zerodds-c-api there already.
if [ -d /zerodds-cache ]; then
  target="/zerodds-cache/cmake-sdk-${CI_CONCURRENT_PROJECT_ID:-0}"
else
  target="${TMPDIR:-/tmp}/zerodds-sdk-build"
fi
work="$(mktemp -d "${TMPDIR:-/tmp}/zerodds-for-each-c.XXXXXX")"
trap 'rm -rf "$work"' EXIT

echo "== build zerodds-c-api =="
(cd "$repo" && cargo build --locked -p zerodds-c-api --target-dir "$target")
lib="$target/debug"

echo "== compile against zerodds.h =="
"${CC:-cc}" -std=c99 -Wall -Wextra -Werror \
  -I "$repo/crates/zerodds-c-api/include" \
  "$repo/crates/zerodds-c-api/tests/for_each_c.c" \
  -L "$lib" -lzerodds -o "$work/for_each_c"

echo "== run on a leased domain =="
out="$(LD_LIBRARY_PATH="$lib:${LD_LIBRARY_PATH:-}" \
  python3 "$repo/tests/integration/with_domains.py" 1 -- \
  sh -c 'exec "$0" "${ZERODDS_TEST_DOMAINS%%,*}"' "$work/for_each_c")"
echo "$out"
grep -q "FOR EACH C OK" <<<"$out" || {
  echo "for_each_c.sh: the program did not report success" >&2
  exit 1
}
