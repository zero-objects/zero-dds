#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# record.sh — the serialized `TypeObject` octets three vendors produce
# for one corpus of types, and the diff against what this tree produces.
#
# ## What this measures and why it is not in the byte gate
#
# `tools/byte-gate` measures the octets a vendor's serialiser writes for
# a *sample*. This measures the octets a vendor's type system writes for
# the *type* — the `TypeObject` of DDS-XTypes 1.3 §7.3.4, whose first
# fourteen MD5 octets are the `EquivalenceHash` two endpoints correlate
# their types by. A byte that moves here does not corrupt a sample; it
# gives the same type two different identities and the peers stop
# recognising each other.
#
# The two are separate drivers because the vendor entry points have
# nothing in common: a sample goes through the generated serialiser, a
# `TypeObject` comes out of the IDL compiler's own type-map blob
# (Cyclone), out of the `TypeObjectRegistry` (Fast DDS) or out of
# `DDS_TypeObjectV2_create_from_typecode` (RTI). The recorded rows also
# live apart: `crates/ast-layout/tests/fixtures/vendor_bytes.rows.tsv`
# is regenerated in full by `tools/byte-gate/verify.sh`, and a row in it
# that that script cannot produce is counted as a vendor that stopped
# answering — which is exactly the false failure the file's own header
# warns about.
#
# ## Where the recorded octets are
#
# `crates/types/tests/typeobject_vendor_dheader.rs`. They are constants
# in a test rather than a TSV beside it, because the comparison then
# runs on every `cargo test` instead of only when someone remembers a
# gate. This script is how the constants are re-derived from the vendors
# when a toolchain moves.
#
# ## Running it
#
#   tools/typeobject-bytes/record.sh                 # against $HOST
#   HOST=<test-host> tools/typeobject-bytes/record.sh
#   HOST=<test-host> tools/typeobject-bytes/record.sh --keep
#
# The vendors run on the Linux test host; nothing here builds on macOS.
# A vendor whose toolchain is not there is named on stderr with the rows
# it did not produce — never skipped in silence.
#
# ## The corpus
#
# `zerodds-examples/idl-conformance/proofs/typeobject/idl/typeobject.idl`
# and the three extractors beside it. The four leaf structures — `Plain`,
# `Appendable`, `Mutable`, `Inner` — are the cases whose MINIMAL hashes
# CycloneDDS and Fast DDS agree on and whose COMPLETE hashes all three
# vendors agree on, so they are a consensus and not one vendor's answer.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
HOST="${HOST:?HOST names the measurement host, reached over ssh}"
REMOTE="${REMOTE:-/root/xtypes-typeobject-bytes}"
PROOF="$ROOT/zerodds-examples/idl-conformance/proofs/typeobject"
KEEP=0
[ "${1:-}" = "--keep" ] && KEEP=1

CYCLONE_HOME="${CYCLONE_HOME:-/opt/cyclone}"
NDDSHOME_REMOTE="${NDDSHOME_REMOTE:-/opt/rti.com/rti_connext_dds-7.7.0}"

for needed in idl/typeobject.idl extractors/cyclone_extract.py extractors/rti_extract.c; do
    [ -r "$PROOF/$needed" ] || {
        echo "record: $PROOF/$needed is not readable." >&2
        echo "  The corpus lives in the zerodds-examples checkout beside this one." >&2
        exit 2
    }
done

echo "record: staging the corpus on $HOST:$REMOTE" >&2
ssh -o BatchMode=yes "root@$HOST" "mkdir -p $REMOTE/cyc $REMOTE/rti_out $REMOTE/rti_res $REMOTE/out" \
    || { echo "record: $HOST is not reachable. Nothing was measured." >&2; exit 2; }
scp -q "$PROOF/idl/typeobject.idl" \
       "$PROOF/extractors/cyclone_extract.py" \
       "$PROOF/extractors/rti_extract.c" \
       "root@$HOST:$REMOTE/" || exit 2

# ------------------------------------------------------------- Cyclone
# `idlc` writes `typeobject.c`, whose `TYPE_MAP_CDR_<T>` array is the
# serialized `TypeMapping` of §7.6.3.2.1. The extractor pulls each
# `TypeObject` out of it *and checks that its own MD5 is the hash
# Cyclone put in the `TypeIdentifier` beside it* — so what comes out is
# the stream Cyclone hashes and not a guess at where the object starts.
#
# Cyclone's generated header is moved aside afterwards: it is called
# `typeobject.h`, and so is RTI's.
echo "record: CycloneDDS" >&2
ssh -o BatchMode=yes "root@$HOST" "
    set -e
    cd $REMOTE
    LD_LIBRARY_PATH=$CYCLONE_HOME/lib $CYCLONE_HOME/bin/idlc -l c typeobject.idl
    python3 cyclone_extract.py typeobject.c out/
    mv -f typeobject.c typeobject.h cyc/
" 2>&1 | sed 's/^/  cyclone: /' >&2 \
    || echo "record: CycloneDDS produced no rows ($CYCLONE_HOME)." >&2

# ----------------------------------------------------------------- RTI
# `create_from_typecode` returns the COMPLETE `TypeObject` and both
# hashes. Its serialized form carries a 24-octet envelope — a length, a
# count, the `EK_COMPLETE` octet and the 14-octet hash — and the object
# itself begins at offset 24. That offset is not assumed below: the
# comparison searches for Cyclone's stream inside RTI's blob and prints
# where it found it, so a release that changes the envelope says so
# rather than reading as a changed object.
echo "record: RTI Connext" >&2
ssh -o BatchMode=yes "root@$HOST" "
    set -e
    cd $REMOTE
    export NDDSHOME=$NDDSHOME_REMOTE
    L=\$(ls -d \$NDDSHOME/lib/x64Linux* | head -1)
    \$NDDSHOME/bin/rtiddsgen -language C -d rti_out -replace typeobject.idl > rtigen.log 2>&1
    gcc -DRTI_UNIX -DRTI_LINUX -DRTI_64BIT rti_extract.c \
        rti_out/typeobject.c rti_out/typeobjectPlugin.c rti_out/typeobjectSupport.c \
        -I rti_out -I \$NDDSHOME/include -I \$NDDSHOME/include/ndds \
        -L \$L -lnddsc -lnddscore -ldl -lm -lpthread -lrt -o rti_extract
    LD_LIBRARY_PATH=\$L ./rti_extract rti_res
" 2>&1 | sed 's/^/  rti: /' >&2 \
    || echo "record: RTI Connext produced no rows ($NDDSHOME_REMOTE)." >&2

# ------------------------------------------------------------ the rows
echo >&2
echo "record: the octets, and where RTI's blob holds Cyclone's stream" >&2
ssh -o BatchMode=yes "root@$HOST" "cd $REMOTE && python3 - <<'PY'
import hashlib, os

CASES = [('Plain', 'plain'), ('Appendable', 'appendable'),
         ('Mutable', 'mutable'), ('Inner', 'inner')]
print('case\tform\tvendor\tlength\thash\tbytes')
for cyc_name, rti_name in CASES:
    for form in ('minimal', 'complete'):
        path = f'out/{cyc_name}.{form}.bin'
        if not os.path.exists(path):
            print(f'{cyc_name}\t{form}\tcyclone\t-\t-\t<no row>')
            continue
        b = open(path, 'rb').read()
        h = hashlib.md5(b).hexdigest()[:28]
        print(f'{cyc_name}\t{form}\tcyclone\t{len(b)}\t{h}\t{b.hex()}')
    rti = f'rti_res/{rti_name}.complete.bin'
    cyc = f'out/{cyc_name}.complete.bin'
    if os.path.exists(rti) and os.path.exists(cyc):
        r, c = open(rti, 'rb').read(), open(cyc, 'rb').read()
        at = r.find(c)
        reported = open(f'rti_res/{rti_name}.complete.hash').read().strip()
        agrees = 'yes' if reported == hashlib.md5(c).hexdigest()[:28] else 'NO'
        print(f'{cyc_name}\tcomplete\trti\t{len(r)}\t{reported}\t'
              f'holds-cyclone-at={at} hash-agrees={agrees}')
    else:
        print(f'{cyc_name}\tcomplete\trti\t-\t-\t<no row>')
PY"

echo >&2
echo "record: compare these against the CASES table of" >&2
echo "  crates/types/tests/typeobject_vendor_dheader.rs" >&2
if [ "$KEEP" -eq 0 ]; then
    ssh -o BatchMode=yes "root@$HOST" "rm -rf $REMOTE" >/dev/null 2>&1
    echo "record: removed $HOST:$REMOTE (--keep leaves it)" >&2
fi
