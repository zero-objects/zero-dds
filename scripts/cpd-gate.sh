#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# cpd-gate.sh — PMD CPD copy/paste gate (the SECOND, token-based detector).
#
# This is the token-based sibling of the line-based `jscpd` gate. The two run
# INDEPENDENTLY on purpose: jscpd (`.jscpd.json`, minTokens 50, line-oriented)
# and PMD CPD (this script, token-oriented) tokenize differently, so each
# catches clones the other misses — notably small same-language clones
# (~30-50 tokens, the C# `ExtensibilityKind` enum class) that slip under
# jscpd's line-based `minTokens`.
#
# Called identically from BOTH CI systems:
#   - .gitlab-ci.yml         (job `cpd`,   stage lint)   — authoritative
#   - .github/workflows/ci.yml (job `cpd`)               — public mirror
# so a single source of truth can't drift between GitLab and GitHub.
#
# ---------------------------------------------------------------------------
# HOW THE GATE DECIDES (per language, ratchet)
#
# CPD has no `%` threshold like jscpd and no native "only new duplication"
# mode. Two knobs are combined per language, both committed in the LANGS
# table below:
#   - minimum-tokens: the smallest clone (in CPD tokens) that counts.
#   - BASELINE:       the number of duplication blocks CPD reports on the
#                     CURRENT tree at that minimum-tokens.
# The gate FAILS a language only when its live block count EXCEEDS the
# committed BASELINE — i.e. when NEW duplication is added on top of the known
# level. Refactors that remove or move duplication keep it GREEN. It is a
# RATCHET: when duplication is cleaned up, lower the BASELINE (and/or the
# minimum-tokens) so the reduced level is locked in. Do NOT raise a BASELINE
# to paper over new copy-paste; de-duplicate or wrap the legitimate block in
# `// CPD-OFF` / `// CPD-ON` markers instead (files under crates/ and
# endpoints/ are owned elsewhere — prefer de-duplication or a BASELINE bump
# with a one-line reason over editing them here).
#
# Why baselines rather than "must be zero": at a useful minimum-tokens the
# tree is NOT clone-free today. Rust carries the idl-* cross-backend code
# emitters (intentional per-backend near-duplicates); C++ carries the
# per-vendor tests/perf/dds-roundtrip-bench harnesses; C# carries binding
# boilerplate. Those live under crates/ and tests/ and are not edited from
# here, so the level is frozen as a baseline and only GROWTH trips the gate.
#
# ---------------------------------------------------------------------------
# RE-BASELINING
#
#   PMD_BIN=/path/to/pmd scripts/cpd-gate.sh --update-baseline
#
# prints the current block count per language; copy the numbers into the
# LANGS table. Re-baseline after an intentional PMD_VERSION bump (a new
# tokenizer shifts counts) or after landing/removing large intentional
# duplication. Counts are deterministic for a fixed PMD_VERSION + file set
# (git ls-files output is sorted), so the committed baselines reproduce.
#
# ---------------------------------------------------------------------------
# PMD BINARY
#
# PMD 7.x is a JVM tool (needs a JRE/JDK on PATH). The `pmd cpd` binary is
# resolved in this order:
#   1. $PMD_BIN, if set and executable.
#   2. `pmd` on $PATH.
#   3. auto-download of pmd-dist-$PMD_VERSION-bin.zip into $PMD_HOME
#      (default /tmp/pmd-$PMD_VERSION) — needs curl + unzip.
# The distribution is NEVER committed; CI fetches it at runtime.
# ---------------------------------------------------------------------------
set -euo pipefail
# Disable filename globbing: the LANGS globs (e.g. `*.py`) are passed to
# `git ls-files`, which does its own recursive matching. Without this a
# top-level match (e.g. ./lp_reliable.py) would make the shell expand `*.py`
# to just that one file before git ever sees it.
set -f

PMD_VERSION="${PMD_VERSION:-7.26.0}"

UPDATE_BASELINE=0
[ "${1:-}" = "--update-baseline" ] && UPDATE_BASELINE=1

# --- exclusion regex (mirrors the `ignore` list in .jscpd.json) -------------
# Matched against repo-relative paths from `git ls-files`. Kept in the same
# spirit as jscpd: build/generated output, snapshots, goldens, lockfiles, the
# release mirror (github/), internal/, content mirrors (docs/, website/, man/),
# teaching duplication (examples/tutorials/), and the compliance corpus.
EXCLUDE_RE='(^|/)target/|(^|/)node_modules/|(^|/)dist/|(^|/)build/|endpoints/[^/]+/build/|(^|/)_generated/|(^|/)snapshots/|/tests/snapshots/|\.snap$|(^|/)goldens/|\.golden$|\.lock$|package-lock\.json$|^github/|^internal/|^docs/|^website/|^man/|^examples/tutorials/|^tests/compliance/'

# --- per-language config: "cpd-language|git-globs|minimum-tokens|baseline" ---
# Rust is the bulk (~636k LOC): minimum-tokens 150 targets duplicated
# functions/modules (jscpd's file/module class) while keeping the idl-*
# emitter noise as a fixed baseline. C# runs at 50 tokens on purpose — it is
# the small-clone case CPD exists to catch. The rest run at 75.
LANGS=(
  "rust|*.rs|150|473"
  "cs|*.cs|50|30"
  "cpp|*.cpp *.cc *.hpp *.h *.c|100|101"
  "go|*.go|75|4"
  "python|*.py|75|3"
  "typescript|*.ts|75|8"
  "kotlin|*.kt *.kts|75|1"
  "swift|*.swift|75|1"
  "lua|*.lua|75|1"
)

# --- resolve the pmd binary -------------------------------------------------
resolve_pmd() {
  if [ -n "${PMD_BIN:-}" ] && [ -x "${PMD_BIN}" ]; then
    echo "${PMD_BIN}"; return 0
  fi
  if command -v pmd >/dev/null 2>&1; then
    command -v pmd; return 0
  fi
  local home="${PMD_HOME:-/tmp/pmd-${PMD_VERSION}}"
  local bin="${home}/pmd-bin-${PMD_VERSION}/bin/pmd"
  if [ ! -x "${bin}" ]; then
    echo "cpd-gate: downloading PMD ${PMD_VERSION} into ${home}" >&2
    mkdir -p "${home}"
    local url="https://github.com/pmd/pmd/releases/download/pmd_releases/${PMD_VERSION}/pmd-dist-${PMD_VERSION}-bin.zip"
    curl -fsSL -o "${home}/pmd.zip" "${url}"
    unzip -q -o "${home}/pmd.zip" -d "${home}"
  fi
  echo "${bin}"
}

repo_root="$(git rev-parse --show-toplevel)"
cd "${repo_root}"

PMD="$(resolve_pmd)"
if [ ! -x "${PMD}" ] && ! command -v "${PMD}" >/dev/null 2>&1; then
  echo "cpd-gate: could not resolve a pmd binary (set PMD_BIN or install pmd)" >&2
  exit 3
fi

status=0
printf '%-12s %8s %8s %8s\n' "language" "files" "blocks" "baseline"
printf '%-12s %8s %8s %8s\n' "--------" "-----" "------" "--------"

for entry in "${LANGS[@]}"; do
  IFS='|' read -r lang globs mt baseline <<<"${entry}"

  filelist="$(mktemp)"
  # shellcheck disable=SC2086
  git ls-files ${globs} | grep -Ev "${EXCLUDE_RE}" > "${filelist}" || true
  nfiles="$(wc -l < "${filelist}" | tr -d ' ')"
  if [ "${nfiles}" -eq 0 ]; then
    rm -f "${filelist}"
    continue
  fi

  report="$("${PMD}" cpd --minimum-tokens "${mt}" --language "${lang}" \
            --file-list "${filelist}" --format text --no-fail-on-error 2>/dev/null || true)"
  blocks="$(printf '%s' "${report}" | grep -c 'Found a ' || true)"
  rm -f "${filelist}"

  if [ "${UPDATE_BASELINE}" -eq 1 ]; then
    printf '%-12s %8s %8s %8s\n' "${lang}" "${nfiles}" "${blocks}" "${baseline} <- suggest ${blocks}"
    continue
  fi

  mark="ok"
  if [ "${blocks}" -gt "${baseline}" ]; then
    mark="FAIL (+$((blocks - baseline)))"
    status=1
  fi
  printf '%-12s %8s %8s %8s  %s\n' "${lang}" "${nfiles}" "${blocks}" "${baseline}" "${mark}"

  # On growth, print the clone report so CI shows WHAT was added.
  if [ "${blocks}" -gt "${baseline}" ]; then
    echo "----- CPD (${lang}) duplication report — ${blocks} blocks, baseline ${baseline} -----"
    printf '%s\n' "${report}"
    echo "----- end CPD (${lang}) report -----"
  fi
done

if [ "${UPDATE_BASELINE}" -eq 1 ]; then
  echo "cpd-gate: --update-baseline (no gating); copy suggested numbers into LANGS"
  exit 0
fi

if [ "${status}" -ne 0 ]; then
  echo "cpd-gate: FAILED — a language exceeds its committed baseline (new copy/paste). See report(s) above; de-duplicate, add // CPD-OFF/CPD-ON around a legitimate block, or lower/raise the baseline in scripts/cpd-gate.sh with a reason." >&2
else
  echo "cpd-gate: OK — no language exceeds its baseline."
fi
exit "${status}"
