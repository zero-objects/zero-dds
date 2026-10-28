#!/usr/bin/env bash
# Canonical crates.io publish driver for a ZeroDDS release.
#
# Replaces the per-rc copies (.publish-rc1.sh, .publish-rc3.sh, …): pass the
# workspace version as the single argument. The publish order comes from
# `zerodds-cargo-dag` (topological sort, --only-publishable) — which derives the
# member set from `cargo metadata`, so it covers the crates that are workspace
# members only because a listed member depends on them by path, not just the
# names spelled out in `workspace.members`. The loop is
# idempotent — a crate whose $VERSION already answers 200 on the crates.io API,
# *or* whose `cargo publish` reports "already exists", is skipped, never a stop.
# Safe to re-run after any stall / rate-limit / partial run.
#
#   scripts/publish-crates.sh 1.0.0-rc.7              # real publish (needs cargo login)
#   scripts/publish-crates.sh 1.0.0-rc.7 --dry-run    # package+metadata check, no upload
#
# Artifacts (.publish-<version>.{order,log}) are written at repo root and are
# gitignored — they are the per-run "rc flag" trail, never committed.
#
# Pre-flight that this loop assumes (see internal/release/release-playbook.md §B):
#   * workspace bumped to $VERSION, intra-workspace deps normalized (Phase A)
#   * no versioned intra-workspace *dev-dependencies* on later-published crates
#     (keep them path-only) — cargo-dag does not order dev-deps
#   * libduckdb crates are `publish = false` (source-only), so cargo-dag drops them
set -u

VERSION="${1:-}"
MODE="${2:-}"
if [ -z "$VERSION" ]; then
  echo "usage: $0 <version> [--dry-run]   e.g. $0 1.0.0-rc.7" >&2
  exit 2
fi
case "$VERSION" in
  [0-9]*.[0-9]*.[0-9]*) : ;;   # looks like a semver
  *) echo "refusing: '$VERSION' does not look like a semver version" >&2; exit 2 ;;
esac

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT" || exit 1
ORDER=".publish-${VERSION}.order"
LOG=".publish-${VERSION}.log"
# crates.io data-access policy requires a descriptive User-Agent on API calls.
UA="zerodds-${VERSION}-publish (https://zerodds.org; admin@zerodds.org)"

echo "[$(date +%T)] generate publish order via cargo-dag…" | tee -a "$LOG"
cargo run -q -p zerodds-cargo-dag -- . --only-publishable --format flat > "$ORDER" 2>>"$LOG" || {
  echo "cargo-dag failed — see $LOG" >&2; exit 1; }
total=$(wc -l < "$ORDER" | tr -d ' '); i=0; pub=0; skip=0; defer=0

if [ "$MODE" = "--dry-run" ]; then
  echo "[$(date +%T)] DRY-RUN — packaging+metadata across $total crates ($VERSION)" | tee -a "$LOG"
  while IFS= read -r crate; do
    i=$((i+1)); echo "($i/$total) $crate" | tee -a "$LOG"
    # --no-verify: skip the verify rebuild (it fails for downstream crates whose
    # $VERSION deps are not on crates.io yet — expected, not a real error). The
    # packaging + metadata gate (description/license/repository/readme/…) is what
    # this catches.
    out=$(cargo publish -p "$crate" --dry-run --allow-dirty --no-verify 2>&1); rc=$?
    if [ $rc -ne 0 ]; then
      # Expected, NOT a real error: a downstream crate has a *versioned*
      # intra-workspace dependency at $VERSION that is not on crates.io yet
      # (a dry-run uploads nothing). The topological real publish resolves this
      # because the dep is published first. Defer it; only hard-fail on a real
      # packaging / metadata problem (missing description/license/readme/…).
      if echo "$out" | grep -qE "failed to select a version for the requirement .zerodds-[a-z0-9-]+ = \"\^?${VERSION}\""; then
        defer=$((defer+1)); echo "($i/$total) defer $crate (intra-dep @ $VERSION not published yet — ok)" | tee -a "$LOG"
        continue
      fi
      # Cargo's second phrasing for the same situation: `no matching package
      # named 'X' found`. It arrives when the dependency has never been on
      # crates.io at any version, where the first phrasing arrives when some
      # other version is there.
      #
      # Not matched blindly. The named package is only a defer if it is
      # itself in this publish order — then the real run publishes it first
      # and the resolution succeeds. A name that is *not* in the order is a
      # dependency on something that does not exist, which is exactly the
      # class this gate is for: it is how a crate depending on an
      # unpublishable one looked before `64c65d784db`, and a typo in a
      # dependency name looks the same.
      missing=$(echo "$out" | sed -nE "s/.*no matching package named .([a-z0-9_-]+). found.*/\1/p" | head -1)
      if [ -n "$missing" ] && grep -qxF "$missing" "$ORDER"; then
        defer=$((defer+1))
        echo "($i/$total) defer $crate (needs $missing, which this order publishes first — ok)" | tee -a "$LOG"
        continue
      fi
      if [ -n "$missing" ]; then
        echo "$out" | tail -15 | tee -a "$LOG"
        echo "[$(date +%T)] DRY-RUN FAILED on $crate — it needs \`$missing\`, which is NOT in the publish order." | tee -a "$LOG"
        echo "  Either that crate carries publish = false, or the dependency name is wrong." | tee -a "$LOG"
        exit 1
      fi
      echo "$out" | tail -15 | tee -a "$LOG"
      echo "[$(date +%T)] DRY-RUN FAILED on $crate — real packaging/metadata error, fix before publish" | tee -a "$LOG"; exit 1
    fi
    pub=$((pub+1))
  done < "$ORDER"
  echo "[$(date +%T)] DRY-RUN OK ✅ — $pub crates fully packaged+metadata-checked, $defer deferred (downstream deps publish in order)" | tee -a "$LOG"
  exit 0
fi

# `cargo publish --allow-dirty` packages what is on disk, and an untracked
# file that .gitignore does not name goes into the .crate with the rest — a
# binary or a log left in a crate directory would ship. The tree must be
# clean before the first upload and stay clean between uploads (a build
# script that writes into its source directory shows up here, too).
require_clean_tree() {
  dirty=$(git status --porcelain --untracked-files=all)
  if [ -n "$dirty" ]; then
    echo "[$(date +%T)] REFUSED $1 — the tree is not clean; every file listed would be packaged or is a local change:" | tee -a "$LOG"
    echo "$dirty" | head -20 | tee -a "$LOG"
    exit 1
  fi
}
require_clean_tree "before the first upload"

echo "[$(date +%T)] PUBLISH $total crates ($VERSION)" | tee -a "$LOG"
while IFS= read -r crate; do
  i=$((i+1))
  http=$(curl -s -A "$UA" -o /dev/null -w "%{http_code}" \
    "https://crates.io/api/v1/crates/$crate/$VERSION" 2>/dev/null || echo 000)
  if [ "$http" = "200" ]; then skip=$((skip+1)); echo "($i/$total) SKIP $crate (already $VERSION)" | tee -a "$LOG"; continue; fi
  require_clean_tree "at $crate"
  echo "($i/$total) PUBLISH $crate (http=$http)" | tee -a "$LOG"
  out=$(cargo publish -p "$crate" --allow-dirty 2>&1); rc=$?
  echo "$out" >> "$LOG"
  if [ $rc -ne 0 ] && echo "$out" | grep -qiE 'rate.?limit|429|too many'; then
    echo "[$(date +%T)] RATE-LIMIT on $crate — sleep 600" | tee -a "$LOG"; sleep 600
    out=$(cargo publish -p "$crate" --allow-dirty 2>&1); rc=$?; echo "$out" >> "$LOG"
  fi
  # Idempotency: a crate already on crates.io (e.g. a per-crate version that
  # differs from $VERSION, so the HTTP pre-check misses it) is NOT a failure.
  if [ $rc -ne 0 ] && echo "$out" | grep -qiE 'already (exists|uploaded)'; then
    echo "($i/$total) SKIP $crate (already on crates.io)" | tee -a "$LOG"
    skip=$((skip+1)); continue
  fi
  if [ $rc -ne 0 ]; then
    echo "[$(date +%T)] FAILED $crate (rc=$rc) — STOP. Last lines:" | tee -a "$LOG"
    echo "$out" | tail -10 | tee -a "$LOG"; exit 1
  fi
  pub=$((pub+1))
  sleep 60   # crates.io steady-state rate limit
done < "$ORDER"
echo "[$(date +%T)] ALL DONE ✅ — published=$pub skipped=$skip total=$total" | tee -a "$LOG"
