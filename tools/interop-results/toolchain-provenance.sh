#!/bin/bash
# toolchain-provenance.sh — what is actually installed, asked of the
# host rather than believed.
#
#   . tools/interop-env.sh
#   tools/interop-results/toolchain-provenance.sh > tests/interop-results/PROVENANCE.tsv
#
# ## Why this exists
#
# The vendor versions in `tools/interop-results/src/systems.rs` are
# string literals. They are printed beside every column of
# `tests/interop-results/SYSTEMS.md` as though they were measured, and
# nothing connects them to the toolchain that produced a row. Upgrade a
# vendor, re-record, and the table keeps claiming the old version — the
# numbers move and the attribution does not.
#
# This asks each toolchain what it is, on the host that has it, and
# writes one row per system. `verify-provenance.sh` compares the answer
# against what `systems.rs` claims, so a drift is a failing check rather
# than a wrong document.
#
# ## What a row is
#
#   system  probed  version  detail  source
#
# `probed` is `yes` when the version came out of the tool, `no` when the
# tool is absent here. An absent tool is recorded rather than skipped:
# a column measured on a host that no longer has the vendor is a column
# nobody can re-derive, and that has to be visible.
set -u

emit() { printf '%s\t%s\t%s\t%s\t%s\n' "$1" "$2" "$3" "$4" "$5"; }

printf '# What each toolchain reports about itself, on this host.\n'
printf '# Produced by tools/interop-results/toolchain-provenance.sh\n'
printf '# host=%s date=%s tree=%s\n' \
    "$(hostname 2>/dev/null || echo unknown)" \
    "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    "$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
printf 'system\tprobed\tversion\tdetail\tsource\n'

# -- Cyclone ------------------------------------------------------------
cyc="${CYCLONE_HOME:-/opt/cyclone}"
if [ -x "$cyc/bin/idlc" ]; then
    v=$("$cyc/bin/idlc" -v 2>&1 | head -1)
    lib=$(ls "$cyc"/lib/libddsc.so.*.*.* 2>/dev/null | head -1 | sed 's/.*libddsc\.so\.//')
    emit cyclone yes "${v:-unreported}${lib:+ / libddsc $lib}" "$cyc" "idlc -v + lib soname"
else
    emit cyclone no "" "$cyc" "not installed"
fi

# -- RTI ----------------------------------------------------------------
if [ -n "${NDDSHOME:-}" ] && [ -x "$NDDSHOME/bin/rtiddsgen" ]; then
    v=$("$NDDSHOME/bin/rtiddsgen" -version 2>&1 | grep -m1 -oE "rtiddsgen version [0-9.]+")
    # rtiddsgen reports itself and not Connext. The product version is
    # the install directory's own name, which is how RTI ships it.
    prod=$(basename "$NDDSHOME" | sed 's/^rti_connext_dds-//')
    emit rti yes "RTI Connext DDS ${prod:-unreported}${v:+, $v}" \
         "$NDDSHOME" "install dir + rtiddsgen -version"
else
    emit rti no "" "${NDDSHOME:-unset}" "not installed"
fi

# -- Fast DDS -----------------------------------------------------------
if [ -n "${FASTDDSGEN:-}" ] && [ -x "${FASTDDSGEN}" ]; then
    # `-version` prints the JVM's banner first; the generator's own line
    # is the one naming it. Taking head -1 records the Java version as
    # Fast DDS's, which is what the first run of this script did.
    v=$("$FASTDDSGEN" -version 2>&1 | grep -m1 -i "fastddsgen version")
    # The generator and the library are versioned apart and the probes
    # use both: fastddsgen writes the type support, libfastdds
    # serialises it. Recording only one attributes a row to half its
    # toolchain.
    lib=$(ls "${FASTDDS_HOME:-/opt/fastdds}"/lib/libfastdds.so.*.*.* 2>/dev/null \
          | head -1 | sed 's/.*libfastdds\.so\.//')
    # Fast CDR is a third version in the same toolchain, and it is the
    # one that decides the octets — the serialiser lives there.
    cdr=$(ls "${FASTDDS_HOME:-/opt/fastdds}"/lib/libfastcdr.so.*.*.* 2>/dev/null \
          | head -1 | sed 's/.*libfastcdr\.so\.//')
    emit fastdds yes "${v:-unreported}${lib:+ / libfastdds $lib}${cdr:+ / Fast CDR $cdr}" \
         "${FASTDDS_HOME:-unset}" "fastddsgen -version + lib sonames"
else
    emit fastdds no "" "${FASTDDSGEN:-unset}" "not installed"
fi

# -- OpenDDS ------------------------------------------------------------
odds="${OPENDDS_HOME:-/opt/opendds}"
if [ -r "$odds/include/dds/Version.h" ]; then
    # `opendds_idl --version` prints its own path and nothing else. The
    # version is a define in the installed header, and ACE's is beside
    # it — both belong in the row, because the pair is what OpenDDS's
    # serialiser is.
    v=$(grep -m1 "define OPENDDS_VERSION " "$odds/include/dds/Version.h" \
        | sed 's/.*"\(.*\)".*/\1/')
    a=$(grep -m1 "define ACE_VERSION " "$odds/include/ace/Version.h" 2>/dev/null \
        | sed 's/.*"\(.*\)".*/\1/')
    emit opendds yes "OpenDDS ${v:-unreported}${a:+ / ACE $a}" "$odds" "include/dds/Version.h"
else
    emit opendds no "" "$odds" "not installed"
fi

# -- the ORBs -----------------------------------------------------------
if command -v omniidl >/dev/null 2>&1; then
    # `omniidl -V` reports the *compiler's* version (1.0), not omniORB's.
    # The library package is what the probes link against.
    v=$(dpkg-query -W -f='${Version}' libomniorb4-dev 2>/dev/null \
        || dpkg-query -W -f='${Version}' 'libomniorb4*' 2>/dev/null | head -1)
    emit omniorb yes "omniORB ${v:-unreported}" "$(command -v omniidl)" "dpkg libomniorb4-dev"
else
    emit omniorb no "" "" "not installed"
fi

if [ -n "${TAO_IDL:-}" ] && [ -x "${TAO_IDL}" ]; then
    emit tao yes "$("$TAO_IDL" -V 2>&1 | head -1)" "$TAO_IDL" "tao_idl -V"
else
    emit tao no "" "${TAO_IDL:-unset}" "not installed"
fi

jac="${JACORB_HOME:-/opt/jacorb}"
if [ -d "$jac" ]; then
    v=$(ls "$jac"/lib/jacorb*.jar 2>/dev/null | head -1 | sed 's/.*jacorb-\?//; s/\.jar$//')
    emit jacorb yes "${v:-unreported}" "$jac" "jar name"
else
    emit jacorb no "" "$jac" "not installed"
fi

# -- ROS ----------------------------------------------------------------
if [ -n "${ROS_DISTRO:-}" ]; then
    emit ros yes "$ROS_DISTRO" "${AMENT_PREFIX_PATH%%:*}" "ROS_DISTRO"
else
    emit ros no "" "" "not sourced"
fi

# -- HDDS ---------------------------------------------------------------
hg="${HDDSGEN:-/root/hdds-probe/hdds_gen_src/target/release/hddsgen}"
if [ -x "$hg" ]; then
    emit hdds yes "$("$hg" --version 2>&1 | head -1)" "$hg" "hddsgen --version"
else
    emit hdds no "" "$hg" "not installed"
fi

# -- this tree ----------------------------------------------------------
emit zerodds yes "$(git describe --tags --always 2>/dev/null || echo unknown)" \
    "$(git rev-parse --short HEAD 2>/dev/null || echo unknown)" "git"
emit rustc yes "$(rustc --version 2>/dev/null | head -1)" "" "rustc --version"
