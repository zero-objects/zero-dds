#!/usr/bin/env bash
# Pre-Commit-Hook fuer ZeroDDS — laeuft fmt/clippy/test/zerodds-lint
# inkl. Fuzz-Workspace (der vom Parent-Workspace disconnected ist).
#
# Aufruf: bash scripts/pre-commit.sh
#
# Siehe feedback_pre_commit_checks.md — vor jedem git push.
set -euo pipefail

cd "$(dirname "$0")/.."
ROOT=$(pwd)

echo "== cargo fmt (workspace + fuzz) =="
cargo fmt --all -- --check
# Fuzz-Workspace ist disconnected (leerer [workspace]-Block in
# crates/rtps/fuzz/Cargo.toml) — separat formatieren.
if [[ -d crates/rtps/fuzz ]]; then
    (cd crates/rtps/fuzz && cargo fmt --all -- --check)
fi

echo "== cargo clippy (workspace) =="
cargo clippy --workspace --all-targets -- -D warnings

echo "== cargo test (workspace) =="
cargo test --workspace --quiet

echo "== zerodds-lint =="
cargo run --quiet -p zerodds-lint --bin zerodds-lint -- check

echo "== no_std-Build zerodds-rtps =="
cargo check --quiet -p zerodds-rtps --no-default-features

echo "All pre-commit checks passed."
