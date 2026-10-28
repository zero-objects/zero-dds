#!/usr/bin/env bash
# Run the OMG dds-xtypes harness (branch feature/xml-driver-enhanced) in
# the six configurations its own workflow runs, one JUnit file each:
#
#   --type-object-version {1,2} x --data-representation {1,2}
#   --enable-typeid-tests --type-object-version {1,2}
#
#   run_configs.sh <suite-dir> <python> <pub-exe> <sub-exe> <domain> <out-dir> [extra harness args]
#
# <suite-dir> is the checkout (types/ and data/ are resolved relative to
# it); <python> has the harness's requirements; <pub-exe>/<sub-exe> are
# the two products. The domain is appended to both command lines as
# `-d <domain>` so that two runs on one host do not share domain 0 and
# the topic `test`; the RTPS port band of that domain is checked empty
# before each run. Everything is sequential.
set -euo pipefail

# Empty ACKNACK of a reader that holds nothing: Fast DDS (and, as recorded, OpenDDS
# and RTI Connext) send it with bitmapBase 0 and numBits 0, which DDSI-RTPS 2.5
# section 8.3.5.5 makes invalid; ZeroDDS by default drops the rest of such a
# datagram (section 8.3.4.1 rule 6), and with it the SEDP data that follows in
# it (measured: sec-diag16/17, every red zerodds->fastdds cell had rejections
# with left_out > 0, every green cell none). The ZeroDDS processes of the
# vendor pairs, writers and readers, therefore accept it
# (wire.accept_empty_acknack_base_zero). This is the only place the runner sets it.
export ZERODDS_WIRE_ACCEPT_EMPTY_ACKNACK_BASE_ZERO=1
# PL_CDR1 parameters with the rounded length. Cyclone DDS (extended header) and
# RTI Connext (short header) write the length of a PL_CDR1 parameter rounded up
# to four, the form of DDSI-RTPS 2.5 section 9.4.2.11; XTypes 1.3 section
# 7.4.1.2.2 has the exact length and the default reader of ZeroDDS refuses the
# other form. This runner meets both vendors, so every pair reads tolerantly; the vendors whose writers use the rounded form (taken from the
# recorded rows: ast-layout Dialect::Cyclone and Dialect::Rti,
# pads_declared_length) get a ZeroDDS reader with wire.pl_cdr1_reading=tolerant;
# Fast DDS and OpenDDS write the exact length. This is the only place the runner
# sets it.
export ZERODDS_WIRE_PL_CDR1_READING=tolerant

suite=$1; python=$2; pub=$3; sub=$4; domain=$5; out=$6; shift 6
extra=("$@")
mkdir -p "$out"
cd "$suite"

band_free() {
  local base=$((7400 + 250 * domain)) hi
  hi=$((base + 249))
  local busy
  busy=$(ss -uln | awk -v b="$base" -v h="$hi" '{n=$5; sub(/.*:/,"",n); if (n+0>=b && n+0<=h) print n}' | sort -u | tr '\n' ' ')
  if [ -n "$busy" ]; then
    echo "domain $domain: RTPS band $base-$hi busy: $busy" >&2
    return 1
  fi
}

run() { # <label> <harness args...>
  local label=$1; shift
  if [ -s "$out/$label.xml" ]; then
    echo "== $label: exists, skipped"
    return
  fi
  band_free
  echo "== $label: $(date -u +%FT%TZ)"
  "$python" interoperability_report.py --pub-exe "$pub -d $domain" --sub-exe "$sub -d $domain" \
    -o "$out/$label.xml" "${extra[@]}" "$@" > "$out/$label.log" 2>&1 || true
  python3 - "$out/$label.xml" <<'EOF'
import sys, xml.etree.ElementTree as ET
r = ET.parse(sys.argv[1]).getroot()
s = r if r.tag == "testsuite" else next(r.iter("testsuite"))
bad = [tc.get("name") for tc in r.iter("testcase") if tc.find("failure") is not None or tc.find("error") is not None]
print(f"   tests={s.get('tests')} failures={s.get('failures')} errors={s.get('errors')}")
for b in bad:
    print(f"   red: {b}")
EOF
}

run tov2-x2 --type-object-version 2 -x 2
run tov2-x1 --type-object-version 2 -x 1
run tov1-x2 --type-object-version 1 -x 2
run tov1-x1 --type-object-version 1 -x 1
run typeid-tov2 --enable-typeid-tests --type-object-version 2
run typeid-tov1 --enable-typeid-tests --type-object-version 1
echo "ALL-DONE $(date -u +%FT%TZ)"
