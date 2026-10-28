#!/usr/bin/env bash
# Run the OMG dds-rtps harness for one product against every vendor
# executable in a directory, both directions, one JUnit file per pair.
#
#   run_vendors.sh <harness-dir> <python> <our-exe> <vendor-dir> <out-dir> [vendor-glob] [domain]
#
# <harness-dir> is a checkout of github.com/omg-dds/dds-rtps (the
# harness resolves test_suite.py relative to its own directory);
# <python> is an interpreter with the harness's requirements installed;
# <our-exe> is the product under test; <vendor-dir> holds the release
# executables named <product>_shape_main_linux; <vendor-glob> narrows
# the set (default: all of them). The vendor's own self-interop run is
# taken as well, so that a red cross-vendor cell can be read against
# what the vendor does alone.
#
# Everything in one invocation is sequential: the suite's cases share
# the topic names, and two runs on one domain would read each other's
# traffic. A <domain> is appended to both executables as `-d N`, so two
# invocations on different domains can run side by side; a case that
# names its own domain (Test_Domain_*) names it after ours and wins.
set -euo pipefail

# The ZeroDDS settings the suite needs against the other vendors
# (wire.accept_empty_acknack_base_zero, wire.pl_cdr1_reading,
# writer.unregister_on_delete, reader.honor_data_max_size_serialized,
# reader.dispose_with_unregister) are
# shape_main's own (suite_settings in src/main.rs, the reasons there and in
# README.md), so that a run without this script, as the official matrix
# starts the binary, has them too. An environment variable that names one
# keeps its value.

harness=$1; python=$2; ours=$3; vendors=$4; out=$5; glob=${6:-*_shape_main_linux}; domain=${7:-}
mkdir -p "$out"
ours_name=$(basename "$ours")
suffix=""
if [ -n "$domain" ]; then
  suffix=" -d $domain"
fi
cd "$harness"

run() { # <label> <pub> <sub>
  local label=$1 pub=$2 sub=$3
  if [ -s "$out/$label.xml" ]; then
    echo "== $label: exists, skipped"
    return
  fi
  echo "== $label: $(date -u +%FT%TZ)"
  "$python" interoperability_report.py -P "$pub" -S "$sub" -o "$out/$label.xml" > "$out/$label.log" 2>&1 || true
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

for v in "$vendors"/$glob; do
  vname=$(basename "$v")
  [ "$vname" = "$ours_name" ] && continue
  run "${ours_name}--pub__${vname}--sub" "$ours$suffix" "$v$suffix"
  run "${vname}--pub__${ours_name}--sub" "$v$suffix" "$ours$suffix"
  run "${vname}--pub__${vname}--sub" "$v$suffix" "$v$suffix"
done
echo "ALL-DONE $(date -u +%FT%TZ)"
