#!/usr/bin/env bash
# scripts/gen-crate-readmes.sh
#
# Generate a templated README.md for every crate that doesn't have one.
# Pulls metadata from each crate's Cargo.toml + lib.rs.
#
# Idempotent: existing README.md files are NOT touched. Re-run after
# adding new crates to fill in the new ones.
#
# Usage:
#   scripts/gen-crate-readmes.sh           # crates/ + tools/
#   scripts/gen-crate-readmes.sh --dry-run # show what would be written
#   scripts/gen-crate-readmes.sh --force   # overwrite existing READMEs

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DRY=0
FORCE=0
for arg in "$@"; do
    case "$arg" in
        --dry-run) DRY=1 ;;
        --force)   FORCE=1 ;;
    esac
done

extract_field () {
    local cargo="$1" field="$2"
    grep -m1 "^${field} *=" "$cargo" \
        | sed -E "s/^${field} *= *\"([^\"]*)\".*/\\1/"
}

extract_safety_class () {
    local libfile="$1"
    [[ -f "$libfile" ]] || { echo "TBD"; return; }
    grep -m1 'Safety classification:' "$libfile" \
        | sed -E 's|.*Safety classification:[[:space:]]*\*\*([A-Z-]+)\*\*.*|\1|' \
        | grep -E '^[A-Z-]+$' || echo "TBD"
}

write_readme () {
    local dir="$1"
    local cargo="$dir/Cargo.toml"
    local readme="$dir/README.md"

    # Skip if exists and not --force.
    # `--force` only overwrites previously auto-generated READMEs
    # (identified by the "auto-generated from `Cargo.toml`" marker)
    # — hand-written READMEs are sacrosanct.
    if [[ -f "$readme" ]]; then
        if [[ $FORCE -eq 0 ]]; then
            return 0
        fi
        if ! grep -q "auto-generated from .Cargo.toml. metadata" "$readme"; then
            echo "skip $readme (hand-written)"
            return 0
        fi
    fi

    local name desc safety
    name="$(extract_field "$cargo" name)"
    desc="$(extract_field "$cargo" description)"

    # safety from lib.rs (or main.rs for binaries).
    if [[ -f "$dir/src/lib.rs" ]]; then
        safety="$(extract_safety_class "$dir/src/lib.rs")"
    elif [[ -f "$dir/src/main.rs" ]]; then
        safety="$(extract_safety_class "$dir/src/main.rs")"
    else
        safety="TBD"
    fi
    [[ -z "$safety" ]] && safety="TBD"

    # Compute relative path back to repo root.
    local rel="${dir#"$ROOT/"}"
    local depth
    depth=$(echo "$rel" | tr / '\n' | grep -c .)
    local backlink
    backlink=$(printf '../%.0s' $(seq 1 "$depth"))

    if [[ $DRY -eq 1 ]]; then
        echo "[dry] would write $readme  ($name, safety=$safety)"
        return 0
    fi

    cat > "$readme" <<EOF
# \`${name}\`

${desc}

Part of [**ZeroDDS**](${backlink}README.md). Safety classification: **${safety}**.

## Status

This README is auto-generated from \`Cargo.toml\` metadata. For
hand-written documentation see the rustdoc on the crate's public
items, or the relevant station in the
[Documentation Trail](${backlink}documentation/README.md).

## Usage

Add to your \`Cargo.toml\`:

\`\`\`toml
[dependencies]
${name} = { path = "../path/to/${dir##*/}" }
# or, when published:
# ${name} = "0.x"
\`\`\`

## Tests

\`\`\`bash
cargo test -p ${name}
\`\`\`

## See also

* [\`docs/architecture/02_architecture.md\`](${backlink}docs/architecture/02_architecture.md) —
  layered crate architecture.
* [\`documentation/02-architecture/components.md\`](${backlink}documentation/02-architecture/components.md) —
  per-crate map (English).
EOF
    echo "wrote $readme"
}

count=0
for cargo in $(find "$ROOT/crates" "$ROOT/tools" -maxdepth 2 -name Cargo.toml | sort); do
    dir="$(dirname "$cargo")"
    write_readme "$dir" && count=$((count+1))
done

echo "==> ${count} crates processed"
