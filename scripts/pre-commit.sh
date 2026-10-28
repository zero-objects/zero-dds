#!/usr/bin/env bash
# Pre-commit hook for ZeroDDS — runs fmt/clippy/test/zerodds-lint,
# including the fuzz workspace (which is disconnected from the parent
# workspace).
#
#   bash scripts/pre-commit.sh
#   bash scripts/pre-commit.sh --paths crates/wire/src/lib.rs …
#
# Any arguments are forwarded to tools/staged-tree-guard.sh. Pass
# `--paths` with the files a `git commit -- <path>…` will name: several
# agents share this checkout and each stages only its own files, so the
# index as a whole is not what such a commit writes.
#
# Run before every git push.
set -euo pipefail

cd "$(dirname "$0")/.."

echo "== cargo fmt (workspace + fuzz) =="
cargo fmt --all -- --check
# The fuzz workspace is disconnected (an empty [workspace] block in
# crates/rtps/fuzz/Cargo.toml) — format it separately.
if [[ -d crates/rtps/fuzz ]]; then
    (cd crates/rtps/fuzz && cargo fmt --all -- --check)
fi

echo "== cargo clippy (workspace) =="
cargo clippy --workspace --all-targets -- -D warnings

echo "== silent-drop guard =="
bash tools/silent-drop-guard.sh

# Everything above this line reads the working tree. This one reads what
# is about to be committed, which is not the same thing: three commits
# this cycle shipped a module and left its `pub mod` declaration
# unstaged, and all four working-tree checks were green over each of
# them.
echo "== staged-tree guard =="
bash tools/staged-tree-guard.sh "$@"

echo "== cargo test (workspace) =="
cargo test --workspace --quiet

echo "== zerodds-lint =="
cargo run --quiet -p zerodds-lint --bin zerodds-lint -- check

echo "== no_std build zerodds-rtps =="
cargo check --quiet -p zerodds-rtps --no-default-features

echo "All pre-commit checks passed."
