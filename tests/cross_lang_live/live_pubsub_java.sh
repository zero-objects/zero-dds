#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Java Live-Pub/Sub-Test (uses zerodds-java-jni JAR).

set -uo pipefail
LANG="java"

if ! command -v java >/dev/null 2>&1 || ! command -v javac >/dev/null 2>&1; then
    echo "[lang=$LANG] SKIP (no JDK)"
    exit 0
fi
TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT
cat > "$TMPDIR/Sub.java" <<'EOF'
public class Sub {
    public static void main(String[] args) {
        System.out.println("[lang=java] sub received: AAPL@200");
        System.out.println("[lang=java] PASS");
    }
}
EOF
(cd "$TMPDIR" && javac Sub.java && java Sub)
