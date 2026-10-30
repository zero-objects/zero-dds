#!/usr/bin/env bash
# packaging/github-actions/render-scoop.sh
# Writes the Scoop manifest with the sha512 of the bundles that
# windows-bundle.sh built. The manifest in the repository carries zero
# hashes; only the copy in the bucket names real ones.
# Called by .github/workflows/publish-distros.yml.
#
# Usage:
#   RELEASE_VERSION=1.0.0-rc.7 render-scoop.sh <bundle-dir> <output-json>
set -euo pipefail
DIST="${1:?bundle dir required}"
OUT_FILE="${2:?output file required}"
python3 - "$DIST" "$OUT_FILE" <<'PY'
import hashlib
import json
import os
from pathlib import Path
import sys

dist, out_file = map(Path, sys.argv[1:])
template = Path("packaging/windows/scoop/zerodds.json")
manifest = json.loads(template.read_text())
version = manifest["version"]
if os.environ.get("RELEASE_VERSION", version) != version:
    raise SystemExit(f"{template} has version {version}, not {os.environ['RELEASE_VERSION']}")
for arch, target in (("64bit", "x86_64-pc-windows-msvc"), ("arm64", "aarch64-pc-windows-msvc")):
    entry = manifest["architecture"][arch]
    name = f"zerodds-{version}-{target}.zip"
    expected = f"https://github.com/zero-objects/zero-dds/releases/download/v{version}/{name}"
    if entry["url"] != expected:
        raise SystemExit(f"{template}: {arch} url is {entry['url']}, expected {expected}")
    bundle = dist / name
    if not bundle.is_file() or bundle.stat().st_size == 0:
        raise SystemExit(f"Missing bundle {bundle}")
    entry["hash"] = "sha512:" + hashlib.sha512(bundle.read_bytes()).hexdigest()
out_file.write_text(json.dumps(manifest, indent=4, ensure_ascii=False) + "\n")
print(f"Rendered {out_file} for {version}")
PY
