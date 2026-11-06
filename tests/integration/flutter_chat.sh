#!/usr/bin/env bash
# The tests of the dds-chat tutorial's Flutter app
# (examples/tutorials/dds-chat/apps/flutter-mobile/test): the dart:ffi
# loader of zerodds-ffi-loader-1.0 §3.6, with two Dart processes exchanging
# samples through libzerodds on a leased DDS domain (live_pubsub_test.dart),
# and the app's codec against octets spelled out from DDS-XTypes 1.3
# (message_codec_test.dart).
#
# Builds libzerodds (zerodds-c-api), takes the app's packages from its
# pubspec.lock, runs `flutter test` under tests/integration/with_domains.py
# and writes its results as JUnit into <results> (ci/flutter_junit.py), the
# directory ci/run_integration.py keeps for the job. Prints
# `ENV ZERODDS_LIB=<libzerodds.so>` for the step after it, the Dart
# subscriber of tests/integration/tests/bindings_ffi.rs, which runs with the
# app's packages.
#
#     bash tests/integration/flutter_chat.sh <results>
set -euo pipefail
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
results="$(mkdir -p "${1:?usage: flutter_chat.sh <results>}" && cd "$1" && pwd)"
cd "$repo"
target_dir="${CARGO_TARGET_DIR:-$repo/target}"

if ! command -v flutter >/dev/null 2>&1; then
    echo "flutter_chat: no flutter on PATH (tests/integration/provision.sh flutter)" >&2
    exit 1
fi
# The SDK is a git checkout unpacked from its archive and owned by whoever
# unpacked it; the flutter tool asks git for its version there, which git
# refuses for a repository of another owner. For these processes only.
export GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=safe.directory GIT_CONFIG_VALUE_0='*'
export FLUTTER_SUPPRESS_ANALYTICS=true CI=true

cargo build --locked -p zerodds-c-api
export ZERODDS_LIB="$target_dir/debug/libzerodds.so"
echo "ENV ZERODDS_LIB=$ZERODDS_LIB"

cd examples/tutorials/dds-chat/apps/flutter-mobile
flutter --version --suppress-analytics
flutter pub get --enforce-lockfile --suppress-analytics
status=0
python3 "$repo/tests/integration/with_domains.py" 1 -- \
    flutter test --suppress-analytics --reporter expanded \
    --file-reporter "json:$results/flutter-tests.json" || status=$?
python3 "$repo/ci/flutter_junit.py" "$results/flutter-tests.json" "$results/junit-flutter.xml"
exit "$status"
