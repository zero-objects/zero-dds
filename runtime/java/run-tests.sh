#!/usr/bin/env bash
# The Java runtime's self-tests. No build tool and no dependency: a target
# with a JDK can run this.
#
#   runtime/java/run-tests.sh
#
# Silent when everything holds; every check prints only what failed.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
out="$here/build/classes"
mkdir -p "$out"

# `--release 8` because the runtime targets Java 8 and nothing here needs
# more. `-Xlint:all -Werror` because a warning in a wire runtime is a
# question nobody answered; `-options` is subtracted from the lint set
# because javac 21 warns that release 8 is obsolete, which is a fact about
# the compiler and not about this code.
# `-encoding UTF-8`: these sources carry `§` in their comments, and
# `javac` without it reads the platform encoding — US-ASCII in a
# container, and then every one of them is an error.
javac --release 8 -Xlint:all,-options -Werror -encoding UTF-8 -d "$out" \
    "$here"/src/main/java/org/zerodds/wire/*.java \
    "$here"/src/test/java/org/zerodds/wire/*.java

# Run from the repository root: the cross-column float128 test reads
# `crates/float128/tests/oracle/patterns.txt`, which is where the Rust, C
# and Python columns' agreement is recorded, and the corpus cross-check
# reads `tests/interop-results/`.
cd "$here/../.."

# binary128: the class Java could not have, against 518 patterns the other
# three columns already agree on.
java -cp "$out" org.zerodds.wire.Float128SelfTest

# The wire runtime itself: alignment at both MAXALIGNs, every primitive in
# both byte orders, the framing forms, and RFC 1321's MD5 vectors.
java -cp "$out" org.zerodds.wire.WireSelfTest

# And against a foreign answer: three pure-primitive corpus cases written
# by hand here and compared byte for byte with what the Rust column
# recorded.
java -cp "$out" org.zerodds.wire.CorpusCrossCheck
