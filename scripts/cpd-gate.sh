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
# tree is NOT clone-free today. Rust carries the cross-backend code emitters
# (intentional per-backend near-duplicates — crates/lang-rules,
# crates/lang-render, tools/wireprobe/*-gen, the crates/idl-* remnant, and
# the per-language test files beside them); C++ carries the per-vendor
# tests/perf/dds-roundtrip-bench harnesses; C# carries binding boilerplate.
# Those live under crates/ and tests/ and are not edited from here, so the
# level is frozen as a baseline and only GROWTH trips the gate.
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
# No download fallback: prepare-images supplies the distribution in CI.
# Local callers must provide PMD_BIN or an already installed pmd.
# ---------------------------------------------------------------------------
set -euo pipefail
# Disable filename globbing: the LANGS globs (e.g. `*.py`) are passed to
# `git ls-files`, which does its own recursive matching. Without this a
# top-level match (e.g. ./lp_reliable.py) would make the shell expand `*.py`
# to just that one file before git ever sees it.
set -f

UPDATE_BASELINE=0
[ "${1:-}" = "--update-baseline" ] && UPDATE_BASELINE=1

# --- exclusion regex (mirrors the `ignore` list in .jscpd.json) -------------
# Matched against repo-relative paths from `git ls-files`. Kept in the same
# spirit as jscpd: build/generated output, snapshots, goldens, lockfiles, the
# release mirror (github/), internal/, content mirrors (docs/, website/, man/),
# teaching duplication (examples/tutorials/), the compliance corpus, and the
# roundtrip bench codecs `idlc generate` wrote (tests/perf/dds-roundtrip-bench/gen/).
EXCLUDE_RE='(^|/)target/|(^|/)node_modules/|(^|/)dist/|(^|/)build/|endpoints/[^/]+/build/|(^|/)_generated/|(^|/)snapshots/|/tests/snapshots/|\.snap$|(^|/)goldens/|\.golden$|\.lock$|package-lock\.json$|^github/|^internal/|^docs/|^website/|^man/|^examples/tutorials/|^tests/compliance/|^tests/perf/dds-roundtrip-bench/gen/'

# --- per-language config: "cpd-language|git-globs|minimum-tokens|baseline|skipped" ---
# `skipped` is the number of files PMD's lexer may leave out of the count; a
# file it cannot read is a file whose copies go unseen.
#
# RUST 847 -> 1130, AND THE SKIP COUNT. The CI run of the Rust CPD had been
# failing for weekly pipelines before this change, and this script,
# which discarded PMD's exit code and messages, counted 0 blocks and passed:
# Rust was not measured. Measured again with PMD 7.27.0, the tree of
# 2e20842e3fe, on which 847 was taken, gives 847 with 64 files skipped by the
# lexer, and 1018 with every file read (scripts/cpd_rust_lexable.py). The
# tree of this change gives 1130. Of the 112 since 2e20842e3fe: +46 in the
# emitter family (tools/wireprobe +35, crates/lang-rules +11), about +40
# between the bridge daemons (daemon/cli.rs, daemon/config.rs, the bin
# mains: copied scaffolding), +11 inside crates/xrce, the rest a few per
# crate. RATCHET 1130 -> 1111: the daemons' flag loop and CliError are
# zerodds_config::cli. RATCHET 1111 -> 1108: the security handshake tests
# share one setup of two authenticating stacks.
#
# CPP 124 -> 61, TYPESCRIPT 11 -> 5. The plain and the rich roundtrip bench
# app are one source per stack, and the bench codecs `idlc generate` wrote
# are excluded as generated code. TypeScript had come down unrecorded.
#
# Rust is the bulk (~910k LOC): minimum-tokens 150 targets duplicated
# functions/modules (jscpd's file/module class) while keeping the per-backend
# emitter noise as a fixed baseline. C# runs at 50 tokens on purpose — it is
# the small-clone case CPD exists to catch. The rest run at 75.
#
# RATCHET, rust 860 -> 857. Two shared test helpers replaced copies:
# `crates/lang-rules/tests/typescript_toolchain/mod.rs` — the rule for
# finding a TypeScript compiler, which stood in five files — and
# `crates/dcps/tests/common/pki.rs` — the identity-CA recipe, which
# stood in three, one of them in another crate. The copies were what
# pushed this to 870 in the first place: both rules had just changed
# (no more `npx tsc`, and a named X.509 v3 extension set), and a rule
# that has to be corrected in five places gets corrected in four. The
# extraction took the ten new blocks and three older ones with them.
#
# WHY THE BASELINES MOVED (rust 473->860, cpp 101->124, go 4->5,
# python 3->9, typescript 8->11). PMD_VERSION did not change: the previous
# numbers reproduce EXACTLY (473/101/4/3/8) when this script's file list and
# minimum-tokens are replayed against the tree the baselines were measured
# on, so the whole delta is content, not tokenizer drift.
#
# The delta is the per-backend emitter family being rebuilt and widened —
# the "intentional per-backend near-duplicates" this table already exists to
# hold. The family used to be crates/idl-* (315 files, 177_801 lines); it is
# now crates/lang-rules + crates/lang-render + tools/wireprobe/*-gen plus the
# crates/idl-* remnant (325 files, 232_938 lines), and tools/wireprobe went
# from one generator crate to twenty, one per target language. Measured
# decomposition of rust's +387:
#
#   +316  wholly inside the emitter family   (274 -> 590 blocks)
#   +  5  family against something outside it (2 -> 7 blocks)
#   + 82  added outside the family, of which
#           30  crates/wire-probe (+ tools/interop-results) — a per-vendor/
#               per-language probe crate that did not exist when the
#               baselines were taken (corba_cpp/corba_java/corba_idl,
#               dump_cpp/dump_ros, fill_cpp/fill_ros/fill_rust, and the
#               csharp/java/typescript _column_gate tests)
#           10  crates/endpoint-e2e — one test file per endpoint language
#            9  crates/dcps — per-QoS and per-security test scaffolds, plus
#               three within-file repeats in runtime/publisher/subscriber
#            5  crates/lang-cst — one CST per target language
#            5  crates/rtps — publication_data <-> subscription_data, the
#               two builtin topic types DDSI-RTPS declares in parallel, and
#               three within-file repeats
#            3  crates/types
#           20  the rest, ~15 crate pairs, none above 2 blocks: the
#               coap/mqtt/websocket/amqp/grpc bridge daemons this file
#               already names, ami4ccm <-> ccm, spatial-dds <-> spatial-ros2
#   - 16  blocks that went away
#         (316 + 5 + 82 - 16 = 387)
#
# The columns of that family are deliberately NOT factored together — see the
# module docs in crates/lang-rules/src/default_*.rs: each language re-derives
# XTypes Table 9 rather than calling a sibling's, "because the columns are
# compared against each other and a shared helper would make them agree about
# a mistake". De-duplicating them would remove the property the family exists
# for, so the level is frozen here instead, which is what this table is for.
#
# The four small languages were checked block by block, not in bulk:
#   cpp +23   5x the RFC 1321 MD5 that DDSI-RTPS 2.5 §9.6.3.8 names for the
#             KeyHash, standing in each shippable runtime on its own
#             (runtime/c, runtime/cpp, crates/zerodds-c-api, crates/cpp) —
#             runtime/cpp/src/md5.cpp writes out why: a column that borrowed
#             it would prove nothing about whether it hashes the same bytes
#             the others do, and a wire runtime that pulled in a crypto
#             dependency to name an instance would not link on the targets
#             this project cares about,
#             11x per-vendor probe/template pairs under tools/ (keyhash-probe,
#             try-construct-gate, byte-gate, corba-oracle, wireprobe),
#             4x within-file repeats — the largest is rpc.hpp's
#             RequesterParams/ReplierParams, two valuetypes DDS-RPC
#             §7.11.1.4.11/.12 declares with the same surface — and 3 test pairs
#   go +1     endpoints/go/surface_test.go: the LE and BE halves of one wire
#             case, expected octets written out per case on purpose
#   python +6 5x the per-protocol capture scripts in tools/wire-capture
#             (extract-giop / extract-rtps / extract-rtps-datagrams /
#             decode-rtps / encap-histogram), 1x a surface_test.py repeat
#   python +2 (9->11, 2026-09-16, OMG-suites merge): two more
#             endpoints/python/surface_test.py wire-probe repeats — the same
#             pid_begin/put/pid_end assertion shape per spec case, legitimate
#             test growth, not the duplicated module/enum this gate guards
#   ts +3     runtime/typescript test fixtures (per spec case C.5.x) and one
#             clone that only moved, crates/idl-ts -> runtime/typescript
# None of them is the duplicated-file/module/enum class this gate was built
# for (the C# ExtensibilityKind failure). cs/kotlin/swift/lua did not move.
#
# WHY THE BASELINES MOVED, 2026-09-11 (rust 847, cs 31). Two steps on the
# same day, both measured by replaying PMD against 060abb0f4b4 and the
# current tree:
#
#   Step 1 — rust 857->860, cs 30->31: nine new file-sets. Six are the
#   defaulted-discriminator column deepening per language (the deliberate
#   family above); one is the second Pub.cs <-> Sub.cs constructor mirror
#   (partition names, NativeQosScope, DpCreate{Publisher,Subscriber}) — a
#   writer/reader mirror whose only removal would be a generic ToNative
#   abstraction over seven lines, which the runtime rejects as over-
#   abstraction. Four were corpus-harness skeletons, NOT held intentional.
#
#   Step 2 — rust 860->847: the harness skeletons moved to
#   crates/lang-rules/tests/support/{corpus,generate}.rs, shared through
#   #[path] includes. repository_root, the corpus case list and the
#   materialise pipeline now live once per gate instead of per file; cpp,
#   java and the C# assembly gate keep their own pipelines (rule catalogue
#   + interface items) and share only the case list. The baseline ratchets
#   down to the measured level. The per-language corpus compilers, refusal
#   tables and minimum-render counts stay per file on purpose.
#
#             clone that only moved, crates/idl-ts -> runtime/typescript
# None of them is the duplicated-file/module/enum class this gate was built
# for (the C# ExtensibilityKind failure). cs/kotlin/swift/lua did not move.
#
# RATCHET cs 31 -> 27, cpp 61 -> 60. The QoS policies of the C# bridge are
# converted once per policy (QosBridge.cs); the PSM headers share the
# matched-handle call, the filter expression and the C++03 `function`
# storage, and spell the accessors of the status, built-in topic and octet
# policy templates once (zerodds/psm/handle_seq.hpp,
# zerodds/psm/expression_parameters.hpp). Python and TypeScript are back at
# their baselines (tools/psm-signature-diff.py, the bounded-wstring check of
# runtime/typescript/src/writer.ts, the reliable pair of ts-node's QoS test).
#
# RATCHET rust 1108 -> 1065 (the tree had grown to 1274). The bubble-up
# listener traits of crates/dcps repeat their callbacks through one macro per
# group; the user writer and reader configs that spelled every member at its
# default end in ..Default::default(); the default emitters of
# crates/lang-rules share the alias walk, the discriminator default, the case
# selection and the fixed-point text (crate::default_common,
# crate::fixed_literal), and the generator tests their parse-lower-resolve
# front (tests/support/resolved.rs).
LANGS=(
  "rust|*.rs|150|1065|0"
  "cs|*.cs|50|27|0"
  "cpp|*.cpp *.cc *.hpp *.h *.c|100|60|0"
  "go|*.go|75|5|0"
  "python|*.py|75|11|0"
  "typescript|*.ts|75|5|0"
  "kotlin|*.kt *.kts|75|1|0"
  "swift|*.swift|75|1|0"
  "lua|*.lua|75|1|0"
)

# --- resolve the pmd binary -------------------------------------------------
resolve_pmd() {
  if [ -n "${PMD_BIN:-}" ] && [ -x "${PMD_BIN}" ]; then
    echo "${PMD_BIN}"; return 0
  fi
  if command -v pmd >/dev/null 2>&1; then
    command -v pmd; return 0
  fi
  echo "cpd-gate: PMD missing; only prepare-images may provision CI tools (or set PMD_BIN locally)" >&2
  return 3
}

repo_root="$(git rev-parse --show-toplevel)"
cd "${repo_root}"

# The Rust run reads every Rust file in one CPD pass. Measured on 2323
# files: a 2 GiB heap ends in OutOfMemoryError, 3 GiB and 4 GiB complete,
# and the JVM's default (a quarter of the memory the container shows) ended
# in OutOfMemoryError on the CI runner. A caller's own PMD_JAVA_OPTS wins.
export PMD_JAVA_OPTS="${PMD_JAVA_OPTS:--Xmx4g}"

PMD="$(resolve_pmd)"
if [ ! -x "${PMD}" ] && ! command -v "${PMD}" >/dev/null 2>&1; then
  echo "cpd-gate: could not resolve a pmd binary (set PMD_BIN or install pmd)" >&2
  exit 3
fi

status=0
printf '%-12s %8s %8s %8s %8s\n' "language" "files" "blocks" "baseline" "skipped"
printf '%-12s %8s %8s %8s %8s\n' "--------" "-----" "------" "--------" "-------"

for entry in "${LANGS[@]}"; do
  IFS='|' read -r lang globs mt baseline skip_baseline <<<"${entry}"

  filelist="$(mktemp)"
  # shellcheck disable=SC2086
  git ls-files ${globs} | grep -Ev "${EXCLUDE_RE}" > "${filelist}" || true
  nfiles="$(wc -l < "${filelist}" | tr -d ' ')"
  if [ "${nfiles}" -eq 0 ]; then
    rm -f "${filelist}"
    continue
  fi

  # PMD's Rust lexer skips every file with a string that ends in an escaped
  # backslash or with the char literal '\\' or '"' (70 files when this was added).
  # It reads a copy in which only those literals are spelled differently,
  # with the same tokens and lines; the report names the real paths.
  lexdir=""
  if [ "${lang}" = "rust" ]; then
    lexdir="$(mktemp -d)"
    python3 "${repo_root}/scripts/cpd_rust_lexable.py" "${filelist}" "${lexdir}" "${filelist}.lex"
    mv "${filelist}.lex" "${filelist}"
  fi

  # PMD's own exit code and messages are kept: with them discarded, a run
  # that died (the Rust run in CI, weeks long) counted 0 blocks and passed.
  errfile="$(mktemp)"
  pmd_rc=0
  report="$("${PMD}" cpd --encoding UTF-8 --minimum-tokens "${mt}" --language "${lang}" \
            --file-list "${filelist}" --format text --no-fail-on-error 2>"${errfile}")" || pmd_rc=$?
  if [ -n "${lexdir}" ]; then
    report="${report//${lexdir}\//}"
    rm -rf "${lexdir}"
  fi
  blocks="$(printf '%s' "${report}" | grep -c 'Found a ' || true)"
  # Files PMD's lexer could not read are left out of the count.
  skipped="$(grep -c 'Skipping file' "${errfile}" || true)"
  rm -f "${filelist}"

  # 0: no duplication, 4: duplication found. Anything else measured nothing,
  # or not every file.
  if [ "${pmd_rc}" -ne 0 ] && [ "${pmd_rc}" -ne 4 ]; then
    echo "cpd-gate: pmd cpd for ${lang} exited ${pmd_rc}; its count is not a measurement:" >&2
    grep -v '^[[:space:]]*at ' "${errfile}" | tail -n 20 >&2 || true
    rm -f "${errfile}"
    status=1
    printf '%-12s %8s %8s %8s %8s  %s\n' "${lang}" "${nfiles}" "-" "${baseline}" "-" "PMD FAILED (exit ${pmd_rc})"
    continue
  fi

  if [ "${UPDATE_BASELINE}" -eq 1 ]; then
    rm -f "${errfile}"
    printf '%-12s %8s %8s %8s %8s\n' "${lang}" "${nfiles}" "${blocks}" "${baseline} <- suggest ${blocks}" "${skip_baseline} <- suggest ${skipped}"
    continue
  fi

  mark="ok"
  if [ "${blocks}" -gt "${baseline}" ]; then
    mark="FAIL (+$((blocks - baseline)))"
    status=1
  fi
  if [ "${skipped}" -gt "${skip_baseline}" ]; then
    mark="${mark}; FAIL skipped +$((skipped - skip_baseline))"
    status=1
    echo "----- CPD (${lang}) files the lexer skipped — ${skipped}, baseline ${skip_baseline} -----"
    grep 'Skipping file' "${errfile}" || true
  fi
  rm -f "${errfile}"
  printf '%-12s %8s %8s %8s %8s  %s\n' "${lang}" "${nfiles}" "${blocks}" "${baseline}" "${skipped}/${skip_baseline}" "${mark}"

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
