/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * One IDL type and one JSON sample through OpenDDS's own `Serializer`.
 *
 * OpenDDS is the fourth DDS implementation of the tree and the only one
 * with CORBA/TAO lineage, so its front end takes constructs the other
 * three cannot parse — `map` above all — and its C++ mapping is the
 * classic one, where a member is assigned rather than set through a
 * setter. That is why the fill statements this template includes are
 * the CORBA C++ ones and not Fast DDS's.
 *
 * Unlike Cyclone's and Fast DDS's templates this one needs no
 * byte-order substitution: `OpenDDS::DCPS::Encoding` takes the
 * endianness as a runtime argument, so one binary answers all four
 * combinations and the probe selects the row the run asked for.
 * `tools/byte-gate/tpl/opendds.cpp` prints the same four rows for the
 * same reason; this one differs only in taking its fill from
 * `wireprobe-fill` rather than from a hand-written `.inc`.
 *
 * `Serializer` writes no encapsulation header, so every byte printed
 * here is body — which is what `--emit payload` means everywhere else
 * in this suite.
 *
 * Nothing here decides a wire question. Whether a member is aligned,
 * whether a parameter list is written and what a delimiter contains is
 * OpenDDS's answer, printed.
 */
#include <cstdio>
#include <cstdlib>
#include <dds/DCPS/Serializer.h>
#include <ace/Message_Block.h>
#include "genTypeSupportImpl.h"

using OpenDDS::DCPS::Encoding;
using OpenDDS::DCPS::Serializer;
using OpenDDS::DCPS::ENDIAN_BIG;
using OpenDDS::DCPS::ENDIAN_LITTLE;

/* One combination. The row is `<encoding> <endian> <hex>` or
 * `<encoding> <endian> !<sentence>`: a refusal is a result and is
 * printed, never left as an absent line that a reader would have to
 * interpret. */
static void dump(const char *encoding, const char *endian, Encoding::Kind kind,
                 OpenDDS::DCPS::Endianness endianness, const @TYPE@ &sample) {
    Encoding enc(kind, endianness);
    /* Sized from the sample rather than fixed: a case whose value is
     * larger than one block would otherwise be serialised into a chain
     * and `rd_ptr()` would print the first link only, which is a
     * truncated payload wearing a vendor's name. */
    size_t room = OpenDDS::DCPS::serialized_size(enc, sample) + 64;
    ACE_Message_Block mb(room);
    Serializer ser(&mb, enc);
    if (!(ser << sample)) {
        printf("%s\t%s\t!OpenDDS refused to serialise the sample\n", encoding, endian);
        return;
    }
    if (mb.cont() != 0) {
        printf("%s\t%s\t!the sample did not fit one block\n", encoding, endian);
        return;
    }
    const size_t length = mb.length();
    printf("%s\t%s\t", encoding, endian);
    for (size_t i = 0; i < length; i++)
        printf("%02x", static_cast<unsigned char>(mb.rd_ptr()[i]));
    printf("\n");
}

int main() {
    @TYPE@ sample;
#include "fill.inc"
    dump("xcdr1", "le", Encoding::KIND_XCDR1, ENDIAN_LITTLE, sample);
    dump("xcdr1", "be", Encoding::KIND_XCDR1, ENDIAN_BIG, sample);
    dump("xcdr2", "le", Encoding::KIND_XCDR2, ENDIAN_LITTLE, sample);
    dump("xcdr2", "be", Encoding::KIND_XCDR2, ENDIAN_BIG, sample);
    return 0;
}
