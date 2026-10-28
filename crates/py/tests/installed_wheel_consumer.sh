#!/usr/bin/env bash
# Internal, offline consumer of previously built distributions. No compilation.
set -euo pipefail
if [[ $# -ne 3 ]]; then
    echo "usage: $0 WHEEL_DIRECTORY IDLC_BINARY DOMAIN" >&2
    exit 2
fi
wheel_dir=$(cd "$1" && pwd)
idlc=$(cd "$(dirname "$2")" && pwd)/$(basename "$2")
domain=$3
repo=$(cd "$(dirname "$0")/../../.." && pwd)
work=$(mktemp -d "${TMPDIR:-/tmp}/zerodds-installed-consumer.XXXXXX")
echo "Consumer artifacts: $work"
python3 -m venv "$work/venv"
python="$work/venv/bin/python"
"$python" -m pip install --no-index --find-links="$wheel_dir" 'zerodds==1.0.0rc7'
"$python" -m pip check
mkdir "$work/generated"
"$idlc" generate --lang python --encoding xcdr2 --out "$work/generated" \
    "$repo/tools/idlc/tests/python-support/support.idl"
cp "$repo/crates/py/tests/generated_descriptor_live.py" "$work/generated/consumer.py"
cd "$work/generated"
# Only generated code is outside the installed environment. Neither binding
# nor wire runtime can be supplied from the repository through PYTHONPATH.
env -u PYTHONPATH "$python" consumer.py "$domain" NestedKey
