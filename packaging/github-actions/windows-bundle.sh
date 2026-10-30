#!/usr/bin/env bash
# packaging/github-actions/windows-bundle.sh
# Assembles the Scoop bundle `zerodds-<version>-<target>.zip` that
# packaging/windows/scoop/zerodds.json installs, from the per-binary
# cargo-dist zips of one Windows target. Nothing is compiled: the executables
# are the ones of the release. Called by release.yml -> github-release.
#
# Usage:
#   windows-bundle.sh --dist dist/artifacts-x86_64-pc-windows-msvc \
#                     --target x86_64-pc-windows-msvc --version 1.0.0-rc.7 \
#                     --out release-extra
set -euo pipefail
DIST=""
TARGET=""
VERSION=""
OUT=""
while [ $# -gt 0 ]; do
    case "$1" in
        --dist)    DIST="$2";    shift 2;;
        --target)  TARGET="$2";  shift 2;;
        --version) VERSION="$2"; shift 2;;
        --out)     OUT="$2";     shift 2;;
        *) echo "unknown arg: $1" >&2; exit 1;;
    esac
done
[ -n "$DIST" ]    || { echo "--dist required" >&2; exit 1; }
[ -n "$TARGET" ]  || { echo "--target required" >&2; exit 1; }
[ -n "$VERSION" ] || { echo "--version required" >&2; exit 1; }
[ -n "$OUT" ]     || { echo "--out required" >&2; exit 1; }

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
mkdir -p "$OUT"
python3 - "$ROOT" "$DIST" "$TARGET" "$VERSION" "$OUT" <<'PY'
import hashlib
from pathlib import Path
import sys
import zipfile

root, dist, target, version, out = sys.argv[1:]
root, dist, out = Path(root), Path(dist), Path(out)

# The executables of packaging/windows/msi/zerodds.wxs. zerodds-perf,
# zerodds-traceability and zerodds-lint are internal and not released.
exes = [
    "zerodds-admin", "zerodds-bench", "zerodds-idlc", "zerodds-spy",
    "zerodds-record", "zerodds-replay", "zerodds-snitch", "zerodds-monitor",
    "zerodds-mq", "zerodds-pcap", "zerodds-xmlc", "zerodds-chaos",
    "zerodds-dashboard", "zerodds-recorder-bridge", "zerodds-ws-bridged",
    "zerodds-mqtt-bridged", "zerodds-coap-bridged", "zerodds-amqp-bridged",
    "zerodds-grpc-bridged", "zerodds-corba-bridged", "zerodds-ros2-shim",
]

found = {}
for archive in sorted(dist.rglob(f"*-{target}.zip")):
    with zipfile.ZipFile(archive) as z:
        for name in z.namelist():
            stem = Path(name).name.removesuffix(".exe")
            if name.endswith(".exe") and stem in exes:
                if stem in found:
                    raise SystemExit(f"{stem}.exe is in {found[stem][0]} and {archive}")
                found[stem] = (archive, name)
missing = [e for e in exes if e not in found]
if missing:
    raise SystemExit(f"Missing in {dist}: " + ", ".join(f"{e}.exe" for e in missing))

# Fixed timestamps, so the same input gives the same zip.
stamp = (1980, 1, 1, 0, 0, 0)

def add(z, arcname, data):
    info = zipfile.ZipInfo(f"zerodds/{arcname}", date_time=stamp)
    info.compress_type = zipfile.ZIP_DEFLATED
    info.external_attr = 0o644 << 16
    z.writestr(info, data)

bundle = out / f"zerodds-{version}-{target}.zip"
with zipfile.ZipFile(bundle, "w") as z:
    for stem in exes:
        archive, name = found[stem]
        with zipfile.ZipFile(archive) as src:
            add(z, f"bin/{stem}.exe", src.read(name))
    for config in sorted((root / "packaging/linux/configs").glob("*.example")):
        add(z, f"share/zerodds/configs/{config.name}", config.read_bytes())
    for script in sorted((root / "packaging/windows/services").glob("*.ps1")):
        add(z, f"share/zerodds/services/{script.name}", script.read_bytes())
    add(z, "LICENSE", (root / "LICENSE").read_bytes())

digest = hashlib.sha512(bundle.read_bytes()).hexdigest()
# The form of the cargo-dist .sha512 files next to it.
(out / f"{bundle.name}.sha512").write_text(f"{digest} *{bundle.name}\n")
print(f"{bundle}: {len(exes)} executables")
PY
