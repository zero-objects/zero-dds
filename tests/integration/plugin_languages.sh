#!/usr/bin/env bash
# Builds libzerodds with the `security` feature where
# crates/zerodds-c-api/tests/plugin_languages.rs looks for it
# (`$CARGO_TARGET_DIR/plugin-languages`), so that the three tests of that file
# (DDS-Security 1.2 clause 11: a plugin of the application in C, C++ and Java
# placed into a participant) find the library built and stay within the 25 s
# of a test. The integration job runs the tests in the step after this one;
# a test takes the library it finds there and builds only where there is none
# (a developer who changed the crate runs this script again).
#
#     bash tests/integration/plugin_languages.sh
set -euo pipefail
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
target="${CARGO_TARGET_DIR:-$repo/target}/plugin-languages"
cd "$repo"
cargo build --locked -p zerodds-c-api --features security --target-dir "$target"
