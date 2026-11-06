#!/usr/bin/env bash
# zerodds-py as its tests import it: the extension module `zerodds._core`
# built by cargo (crates/py/build.rs sets what PyO3's extension-module feature
# needs, so no maturin), staged beside the pure-Python package
# crates/py/python/zerodds; runtime/python's zerodds_wire, which the generated
# descriptors import; and libzerodds of zerodds-c-api for the ctypes loader
# (`zerodds.loader`, ZERODDS_LIB). PYTHONPATH and ZERODDS_LIB for the steps
# after it go out as `ENV` lines (ci/run_integration.py). The wheel as a user installs it is the job
# integration-python-wheel.
#
#     bash tests/integration/python_package.sh
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
target="${CARGO_TARGET_DIR:-$repo/target}"

echo "== cargo build -p zerodds-py --features extension-module; -p zerodds-c-api =="
(cd "$repo" && cargo build --locked -p zerodds-py --features extension-module --lib)
(cd "$repo" && cargo build --locked -p zerodds-c-api --lib)

site="$target/python-site"
rm -rf "$site"
mkdir -p "$site"
cp -R "$repo/crates/py/python/zerodds" "$site/zerodds"
suffix="$(python3 -c 'import sysconfig; print(sysconfig.get_config_var("EXT_SUFFIX"))')"
cp "$target/debug/libzerodds_py.so" "$site/zerodds/_core$suffix"
PYTHONPATH="$site:$repo/runtime/python" python3 -c \
  'import zerodds, zerodds_wire; assert zerodds._CORE_AVAILABLE, "zerodds._core did not load"; print("zerodds", zerodds.__version__)'
echo "ENV PYTHONPATH=$site:$repo/runtime/python"
echo "ENV ZERODDS_LIB=$target/debug/libzerodds.so"
