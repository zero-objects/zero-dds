#!/usr/bin/env bash
# A generated type through the C API's typed path, as a C user meets it:
# `zerodds-idlc generate --lang c` writes `<stem>_typesupport.h` for
# crates/zerodds-c-api/tests/generated/typed_c_probe.idl, one
# `zerodds_typesupport_t` per topic type; crates/zerodds-c-api/tests/
# typed_c_roundtrip.c writes three keyed samples through
# `zerodds_writer_write_typed` and takes them back through
# `zerodds_reader_take_typed` in one runtime, compares each with what was
# written and releases it with the table's `sample_free`.
#
# The oracle is the C compiler (`-std=c99 -Wall -Wextra -Werror` over the
# generated header and the C runtime under runtime/c) and the program's own
# comparison of every field. The header is generated on every run, so nothing
# generated is committed to go stale.
#
# This was crates/zerodds-c-api/tests/typed_c_via_cargo.rs; its build of
# zerodds-c-api and zerodds-idlc does not fit the 25 s a test of the crate tier
# has (pipeline 8622 job 227850, pipeline 8643 job 227926: terminated after
# 25 s), so it runs here, beside the SDK it belongs to.
#
#     bash tests/integration/typed_c.sh
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# The target directory of tests/integration/sdk_cmake.sh where the runner
# offers a cache: CMake has built zerodds-c-api there already.
if [ -d /zerodds-cache ]; then
  target="/zerodds-cache/cmake-sdk-${CI_CONCURRENT_PROJECT_ID:-0}"
else
  target="${TMPDIR:-/tmp}/zerodds-sdk-build"
fi
work="$(mktemp -d "${TMPDIR:-/tmp}/zerodds-typed-c.XXXXXX")"
trap 'rm -rf "$work"' EXIT

echo "== build zerodds-c-api and zerodds-idlc =="
(cd "$repo" && cargo build --locked -p zerodds-c-api -p zerodds-idlc --target-dir "$target")
lib="$target/debug"

echo "== zerodds-idlc generate --lang c =="
"$lib/zerodds-idlc" generate --lang c --out "$work" \
  "$repo/crates/zerodds-c-api/tests/generated/typed_c_probe.idl"

echo "== compile the round trip against the generated header =="
runtime="$repo/runtime/c"
"${CC:-cc}" -std=c99 -Wall -Wextra -Werror \
  -I "$work" -I "$runtime/include" -I "$repo/crates/zerodds-c-api/include" \
  "$repo/crates/zerodds-c-api/tests/typed_c_roundtrip.c" \
  "$runtime/src/writer.c" "$runtime/src/reader.c" "$runtime/src/md5.c" \
  "$runtime/src/float128.c" "$runtime/src/float128_decimal.c" \
  -L "$lib" -lzerodds -lm -o "$work/typed_c_roundtrip"

echo "== run on a leased domain =="
out="$(LD_LIBRARY_PATH="$lib:${LD_LIBRARY_PATH:-}" \
  python3 "$repo/tests/integration/with_domains.py" 1 -- \
  sh -c 'exec "$0" "${ZERODDS_TEST_DOMAINS%%,*}"' "$work/typed_c_roundtrip")"
echo "$out"
grep -q "TYPED C ROUNDTRIP OK" <<<"$out" || {
  echo "typed_c.sh: the round trip did not report success" >&2
  exit 1
}
