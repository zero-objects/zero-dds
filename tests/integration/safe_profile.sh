#!/usr/bin/env bash
# The safe-profile gates of docs/architecture/04_safety_by_architecture.md §3
# as the document states them:
#
#   §3.1  cargo build -p safe-crates-only --no-default-features --features safety
#   §3.2  cargo clippy -p <safe-crate> --features safety \
#           -- -D clippy::unwrap_used -D clippy::panic -D clippy::unreachable
#
# The safe crates are the dependencies of crates/safe-crates-only, which is
# the profile's definition in the build. `zerodds-wire` declares no features
# (it is no_std unconditionally), so its clippy run has no `--features`. One
# cargo invocation per crate: `-p a -p b` would unify their features.
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo"

echo "== §3.1 safe-profile build =="
cargo build --locked -p safe-crates-only --no-default-features --features safety

crates="$(python3 - <<'EOF'
import tomllib
manifest = tomllib.load(open("crates/safe-crates-only/Cargo.toml", "rb"))
print(" ".join(sorted(manifest["dependencies"])))
EOF
)"

failed=()
for crate in $crates; do
  dir="$(python3 - "$crate" <<'EOF'
import sys, tomllib
manifest = tomllib.load(open("crates/safe-crates-only/Cargo.toml", "rb"))
print(manifest["dependencies"][sys.argv[1]]["path"])
EOF
)"
  features=()
  if python3 - "crates/safe-crates-only/$dir/Cargo.toml" <<'EOF'
import sys, tomllib
manifest = tomllib.load(open(sys.argv[1], "rb"))
sys.exit(0 if "safety" in manifest.get("features", {}) else 1)
EOF
  then
    features=(--features safety)
  fi
  echo "== §3.2 clippy $crate ${features[*]} =="
  if ! cargo clippy --locked -p "$crate" "${features[@]}" -- \
      -D clippy::unwrap_used -D clippy::panic -D clippy::unreachable; then
    failed+=("$crate")
  fi
done

if [ ${#failed[@]} -gt 0 ]; then
  echo "safe profile: clippy failed for ${failed[*]}" >&2
  exit 1
fi
echo "safe profile: build and the clippy gates of $(echo "$crates" | wc -w) crates passed"
