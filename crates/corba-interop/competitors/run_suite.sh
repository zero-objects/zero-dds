#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# ============================================================================
# CORBA test + interop master suite
# ============================================================================
# A complete suite for the whole CORBA crate family:
#   1. Unit/integration tests of all CORBA crates (cargo test, per crate).
#   2. Codegen E2E (generated stub→IIOP→skeleton, full feature matrix +
#      live NameService) — part of zerodds-corba-interop.
#   3. Cross-ORB interop against omniORB/TAO/JacORB (run_interop.sh).
#
# Run on the Linux test host (Debian 13, omniORB/TAO/JacORB installed). Exit 0 =
# all runnable crates green AND cross-ORB matrix green.
set -uo pipefail
cd "$(dirname "$0")"
HERE="$(pwd)"
ROOT="$(cd ../../.. && pwd)"

# CORBA crate family incl. all language generators. idl-cpp/idl-java were
# previously blocked via zerodds-rpc → zerodds-dcps (default build); since the
# dcps gate fix (prepare_endpoint_crypto_tokens back to #[cfg(security)]) they
# build and test again.
CRATES=(
  zerodds-idl zerodds-idl-rust zerodds-idl-csharp zerodds-idl-ts zerodds-idl-python
  zerodds-idl-cpp zerodds-idl-java
  zerodds-cdr
  zerodds-corba-giop zerodds-corba-iiop zerodds-corba-ior zerodds-corba-poa
  zerodds-corba-rust zerodds-corba-interop
  zerodds-corba-cosnaming zerodds-corba-csiv2 zerodds-corba-ir
  zerodds-corba-cos-event zerodds-corba-cos-notify zerodds-corba-dds-bridge zerodds-corba-dnc
  zerodds-corba-ccm zerodds-corba-ccm-ejb zerodds-corba-ccm-lib zerodds-corba-codegen
  zerodds-ccm zerodds-ami4ccm
)

RC=0
TOTAL_TESTS=0
declare -a ROWS

echo "════════════════════════════════════════════════════════════════"
echo " 1. UNIT/INTEGRATION TESTS (CORBA crate family)"
echo "════════════════════════════════════════════════════════════════"
for c in "${CRATES[@]}"; do
  # Robust detection via the cargo exit code (not via substring grep —
  # error-path tests/debug output harmlessly contain "error[").
  log=$(mktemp)
  if (cd "$ROOT" && cargo test -p "$c" >"$log" 2>&1); then
    n=$(grep -oE "test result: ok\. [0-9]+ passed" "$log" | grep -oE "^[0-9]+|[0-9]+ passed" \
        | grep -oE "[0-9]+" | awk '{s+=$1} END {print s+0}')
    TOTAL_TESTS=$((TOTAL_TESTS + n))
    ROWS+=("  OK      $c ($n tests)")
  else
    ROWS+=("  FAIL    $c — $(grep -m1 -E 'error\[|could not compile|test result: FAILED' "$log")")
    RC=1
  fi
  rm -f "$log"
done
printf '%s\n' "${ROWS[@]}"
echo "  ── Total runnable: $TOTAL_TESTS tests green"

echo
echo "════════════════════════════════════════════════════════════════"
echo " 2. CODEGEN E2E (stub→IIOP→skeleton, full feature matrix + NameService)"
echo "════════════════════════════════════════════════════════════════"
if (cd "$ROOT" && cargo test -p zerodds-corba-interop --test codegen_roundtrip 2>&1 | tail -6); then
  echo "  codegen_roundtrip: see above"
else
  RC=1
fi

echo
echo "════════════════════════════════════════════════════════════════"
echo " 3. CROSS-ORB INTEROP (ZeroDDS ↔ omniORB / TAO / JacORB)"
echo "════════════════════════════════════════════════════════════════"
bash "$HERE/run_interop.sh" || RC=1

echo
echo "════════════════════════════════════════════════════════════════"
[ "$RC" -eq 0 ] && echo " MASTER SUITE: GREEN" || echo " MASTER SUITE: ERROR (RC=$RC)"
echo "════════════════════════════════════════════════════════════════"
exit $RC
