#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Domain-argument contract test (#28): the domain is EXPLICIT ephemeral
# orchestration state (a CLI argument, never an env var, never persisted
# config). Every client role — ZeroDDS reader, ZeroDDS writer, Cyclone client,
# Fast DDS client — must implement the identical contract:
#   * argument omitted        -> documented manual default 100
#   * present + valid 0..=232  -> that exact domain, reported in its log
#   * malformed / negative / out-of-range -> diagnostic + non-zero exit
# A missing expected vendor binary/module is a FAIL, never a SKIP/PASS.
#
# Needs the built ZeroDDS reader/writer, fastdds_robot, and a Cyclone PYBIN;
# runs in the interop CI job where those + the vendor LD paths exist.
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$HERE/../../../.." && pwd)"
ZR="$REPO_ROOT/interop/cyclone-xtypes-27/reader/target/debug/reader"
ZW="$REPO_ROOT/interop/cyclone-xtypes-27/reader/target/debug/writer"
FR="${FASTDDS_ROBOT:-$HERE/../fastdds/build/fastdds_robot}"
CYC="$REPO_ROOT/interop/cyclone-xtypes-27/writers"
PYBIN="${PYBIN:-python3}"
fails=0
fail() { echo "FAIL: $1" >&2; fails=$((fails + 1)); }
rep_domain() { grep -aoE "domain=[0-9]+" | head -1 | cut -d= -f2; }

# valid <role> <expected> -- <cmd...>
valid() {
  local role="$1" exp="$2"; shift 3
  local got; got="$(timeout 8 "$@" 2>&1 | rep_domain)"
  [ "$got" = "$exp" ] || fail "$role: requested domain $exp but reported ${got:-none}"
}
# bad <role> -- <cmd...>   (must exit non-zero)
bad() {
  local role="$1"; shift 2
  if timeout 8 "$@" >/dev/null 2>&1; then fail "$role: invalid domain did not exit non-zero"; fi
}

# ---- required binaries/modules present (missing == FAIL, never skip) --------
[ -x "$ZR" ] || fail "ZeroDDS reader binary missing at $ZR"
[ -x "$ZW" ] || fail "ZeroDDS writer binary missing at $ZW"
[ -x "$FR" ] || fail "fastdds_robot binary missing at $FR"
"$PYBIN" -c "import cyclonedds" 2>/dev/null || fail "cyclonedds not importable via PYBIN=$PYBIN"
[ "$fails" -eq 0 ] || { echo "FAIL: prerequisites missing ($fails)"; exit 1; }

# ---- per role: valid + omitted + malformed + negative + out-of-range --------
# ZeroDDS reader
valid "zerodds-reader" 177 -- "$ZR" 1 177
valid "zerodds-reader" 100 -- "$ZR" 1
for v in abc -5 300; do bad "zerodds-reader($v)" -- "$ZR" 1 "$v"; done
# ZeroDDS writer
valid "zerodds-writer" 178 -- "$ZW" 1 178
valid "zerodds-writer" 100 -- "$ZW" 1
for v in abc -5 300; do bad "zerodds-writer($v)" -- "$ZW" 1 "$v"; done
# Cyclone reader
valid "cyclone-reader" 179 -- "$PYBIN" "$CYC/cyclone_reader.py" appendable 1 179
valid "cyclone-reader" 100 -- "$PYBIN" "$CYC/cyclone_reader.py" appendable 1
for v in abc -5 300; do bad "cyclone-reader($v)" -- "$PYBIN" "$CYC/cyclone_reader.py" appendable 1 "$v"; done
# Cyclone writer
valid "cyclone-writer" 180 -- "$PYBIN" "$CYC/cyclone_writer.py" appendable xcdr2 1 180
valid "cyclone-writer" 100 -- "$PYBIN" "$CYC/cyclone_writer.py" appendable xcdr2 1
for v in abc -5 300; do bad "cyclone-writer($v)" -- "$PYBIN" "$CYC/cyclone_writer.py" appendable xcdr2 1 "$v"; done
# Fast DDS (sub + pub)
valid "fastdds-sub" 181 -- "$FR" sub 1 181
valid "fastdds-sub" 100 -- "$FR" sub 1
for v in abc -5 300; do bad "fastdds-sub($v)" -- "$FR" sub 1 "$v"; done
valid "fastdds-pub" 182 -- "$FR" pub 1 182
for v in abc 300; do bad "fastdds-pub($v)" -- "$FR" pub 1 "$v"; done

[ "$fails" -eq 0 ] && echo "PASS: domain argument contract holds for all four roles" || { echo "FAIL: $fails domain-contract violation(s)"; exit 1; }
