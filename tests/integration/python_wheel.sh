#!/usr/bin/env bash
# The wheels of zerodds (maturin, crates/py) and zerodds-wire (runtime/python)
# as a user installs them: crates/py/tests/installed_wheel_consumer.sh installs
# both into a venv without the repository on its path, checks the dependency
# with pip, generates a type with zerodds-idlc and runs a writer and reader of
# it on a leased domain. What the old suite's job `python-tests` did after its
# pytest run; the pytest suite itself is the job integration-python.
#
#     bash tests/integration/python_wheel.sh
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
target="${CARGO_TARGET_DIR:-$repo/target}"
wheels="$target/python-wheels"
rm -rf "$wheels"
mkdir -p "$wheels"

echo "== the zerodds-wire wheel =="
python3 -m pip wheel --no-deps --wheel-dir "$wheels" "$repo/runtime/python"
echo "== the zerodds wheel (maturin) =="
(cd "$repo/crates/py" && maturin build --features extension-module --out "$wheels")
echo "== zerodds-idlc =="
(cd "$repo" && cargo build --locked -p zerodds-idlc)

echo "== the installed wheels on a leased domain =="
python3 "$repo/tests/integration/with_domains.py" 1 -- \
  sh -c 'exec bash "$0" "$1" "$2" "${ZERODDS_TEST_DOMAINS%%,*}"' \
  "$repo/crates/py/tests/installed_wheel_consumer.sh" "$wheels" "$target/debug/zerodds-idlc"
